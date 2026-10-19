// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {
    IERC20,
    IUniswapV2Router,
    IAerodromeRouter,
    IUniswapV3SwapRouter,
    ISwapRouter02,
    ICurvePool,
    IAaveV3Pool,
    IFlashLoanSimpleReceiver,
    IBalancerVault,
    IBalancerFlashLoanRecipient
} from "./interfaces/IDexRouters.sol";

/// @title HydraAtomicExecutor
/// @notice Institutional-grade atomic multi-hop DEX arbitrage router with Aave V3 and Balancer FlashLoan integration
///         for Hydra-HFT v4.0 Node C (Frankfurt: Base Mainnet & Arbitrum One).
/// @dev Enforces strict zero-principal-loss invariant: reverts the entire transaction if
///      `balanceAfter < balanceBefore + minNetProfit`.
contract HydraAtomicExecutor is IFlashLoanSimpleReceiver, IBalancerFlashLoanRecipient {
    // =========================================================================
    // Custom Errors
    // =========================================================================
    error UnauthorizedCaller(address caller);
    error UnauthorizedFlashLoanCallback(address caller);
    error InvalidFlashLoanInitiator(address initiator);
    error AssetMismatch(address expected, address received);
    error InvalidArrayLength();
    error ZeroAddress();
    error InsufficientAtomicProfit(uint256 balanceBefore, uint256 balanceAfter, uint256 minNetProfit);
    error UnsupportedDexType(uint8 dexType);
    error EmptySwapPath();
    error TokenTransferFailed(address token, address to, uint256 amount);
    error EthTransferFailed(address to, uint256 amount);

    // =========================================================================
    // Events
    // =========================================================================
    event ArbitrageExecuted(
        address indexed baseToken,
        uint256 amountIn,
        uint256 netProfit,
        uint256 timestamp
    );

    event FlashArbitrageExecuted(
        address indexed baseToken,
        uint256 loanAmount,
        uint256 flashFee,
        uint256 netProfit,
        uint256 timestamp
    );

    event ExecutorUpdated(address indexed previousExecutor, address indexed newExecutor);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event EmergencyWithdrawal(address indexed token, address indexed to, uint256 amount);

    // =========================================================================
    // Types & Constants
    // =========================================================================
    uint8 public constant DEX_UNIV2_VOLATILE = 0;
    uint8 public constant DEX_UNIV3 = 1;
    uint8 public constant DEX_CURVE_STABLE = 2;
    uint8 public constant DEX_AERODROME_ROUTER = 3;
    uint8 public constant DEX_UNIV3_ROUTER02 = 4;

    uint8 public constant FLASH_PROVIDER_AAVE_V3 = 0;
    uint8 public constant FLASH_PROVIDER_BALANCER = 1;

    struct SwapStep {
        uint8 dexType;        // 0 = UniV2, 1 = UniV3, 2 = Curve, 3 = Aerodrome (Base), 4 = SwapRouter02 (Base UniV3)
        address poolOrRouter; // Target pool or router address
        address tokenIn;
        address tokenOut;
        uint24 feeTier;       // For UniV3 (100/500/3000) or Aerodrome (0 = volatile, 1 = stable)
        int128 curveIndexIn;  // For Curve pools
        int128 curveIndexOut;
    }

    struct FlashCallbackData {
        address baseToken;
        uint256 preLoanBalance;
        uint256 internalContribution;
        uint256 minNetProfit;
        SwapStep[] steps;
    }

    // =========================================================================
    // Immutable & State Variables
    // =========================================================================
    address public owner;
    address public executor;
    address public immutable aaveV3Pool;
    address public immutable balancerVault;

    // Transient flash-loan context guard
    bool private _inFlashLoan;

    // =========================================================================
    // Modifiers
    // =========================================================================
    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert UnauthorizedCaller(msg.sender);
        }
        _;
    }

    modifier onlyExecutor() {
        if (msg.sender != executor && msg.sender != owner) {
            revert UnauthorizedCaller(msg.sender);
        }
        _;
    }

    // =========================================================================
    // Constructor
    // =========================================================================
    constructor(address _executor, address _aaveV3Pool, address _balancerVault) {
        owner = msg.sender;
        executor = _executor == address(0) ? msg.sender : _executor;
        aaveV3Pool = _aaveV3Pool;
        balancerVault = _balancerVault;
        emit OwnershipTransferred(address(0), msg.sender);
        emit ExecutorUpdated(address(0), executor);
    }

    // =========================================================================
    // Administration & Gas Warmup
    // =========================================================================
    function setExecutor(address _newExecutor) external onlyOwner {
        emit ExecutorUpdated(executor, _newExecutor);
        executor = _newExecutor;
    }

    function transferOwnership(address _newOwner) external onlyOwner {
        require(_newOwner != address(0), "Hydra: Zero owner");
        emit OwnershipTransferred(owner, _newOwner);
        owner = _newOwner;
    }

    /// @notice Pre-approves routers/pools for `type(uint256).max` so hot-path arbitrage swaps skip `SSTORE` approval gas.
    function preApproveTokens(
        address[] calldata tokens,
        address[] calldata spenders
    ) external onlyExecutor {
        require(tokens.length == spenders.length, "Hydra: length mismatch");
        for (uint256 i = 0; i < tokens.length; ) {
            _safeApprove(tokens[i], spenders[i], type(uint256).max);
            unchecked { ++i; }
        }
    }

    // =========================================================================
    // Core Atomic Execution (Internal Capital)
    // =========================================================================
    /// @notice Executes an atomic multi-hop arbitrage loop funded by internal contract capital.
    function executeAtomicArbitrage(
        address baseToken,
        uint256 amountIn,
        uint256 minNetProfit,
        SwapStep[] calldata steps
    ) external onlyExecutor returns (uint256 netProfit) {
        if (steps.length == 0) {
            revert EmptySwapPath();
        }

        uint256 balanceBefore = IERC20(baseToken).balanceOf(address(this));
        require(balanceBefore >= amountIn, "Hydra: Insufficient internal capital");

        uint256 currentAmount = amountIn;
        for (uint256 i = 0; i < steps.length; ) {
            currentAmount = _executeSingleStep(steps[i], currentAmount);
            unchecked { ++i; }
        }

        uint256 balanceAfter = IERC20(baseToken).balanceOf(address(this));
        if (balanceAfter < balanceBefore + minNetProfit) {
            revert InsufficientAtomicProfit(balanceBefore, balanceAfter, minNetProfit);
        }
        netProfit = balanceAfter - balanceBefore;
        emit ArbitrageExecuted(baseToken, amountIn, netProfit, block.timestamp);
    }

    // =========================================================================
    // FlashLoan Fallback Execution (Aave V3 & Balancer Vault)
    // =========================================================================
    /// @notice Initiates a FlashLoan-backed atomic multi-hop arbitrage when pool depth exceeds internal capital.
    /// @param provider 0 = Aave V3 (`flashLoanSimple`), 1 = Balancer Vault (`flashLoan` — 0 bps fee!).
    function executeFlashArbitrage(
        uint8 provider,
        address baseToken,
        uint256 loanAmount,
        uint256 internalContribution,
        uint256 minNetProfit,
        SwapStep[] calldata steps
    ) external onlyExecutor returns (uint256 netProfit) {
        if (steps.length == 0) {
            revert EmptySwapPath();
        }

        uint256 balanceBefore = IERC20(baseToken).balanceOf(address(this));
        require(balanceBefore >= internalContribution, "Hydra: Insufficient internal contribution");

        FlashCallbackData memory callbackData = FlashCallbackData({
            baseToken: baseToken,
            preLoanBalance: balanceBefore,
            internalContribution: internalContribution,
            minNetProfit: minNetProfit,
            steps: steps
        });

        bytes memory encodedParams = abi.encode(callbackData);

        _inFlashLoan = true;
        if (provider == FLASH_PROVIDER_AAVE_V3) {
            IAaveV3Pool(aaveV3Pool).flashLoanSimple(
                address(this),
                baseToken,
                loanAmount,
                encodedParams,
                0
            );
        } else if (provider == FLASH_PROVIDER_BALANCER) {
            address[] memory tokens = new address[](1);
            tokens[0] = baseToken;
            uint256[] memory amounts = new uint256[](1);
            amounts[0] = loanAmount;
            IBalancerVault(balancerVault).flashLoan(
                address(this),
                tokens,
                amounts,
                encodedParams
            );
        } else {
            _inFlashLoan = false;
            revert UnsupportedDexType(provider);
        }
        _inFlashLoan = false;

        uint256 balanceAfter = IERC20(baseToken).balanceOf(address(this));
        if (balanceAfter < balanceBefore + minNetProfit) {
            revert InsufficientAtomicProfit(balanceBefore, balanceAfter, minNetProfit);
        }
        netProfit = balanceAfter - balanceBefore;
    }

    /// @inheritdoc IFlashLoanSimpleReceiver
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external override returns (bool) {
        if (!_inFlashLoan || msg.sender != aaveV3Pool) {
            revert UnauthorizedFlashLoanCallback(msg.sender);
        }
        if (initiator != address(this)) {
            revert InvalidFlashLoanInitiator(initiator);
        }

        FlashCallbackData memory data = abi.decode(params, (FlashCallbackData));
        if (asset != data.baseToken) {
            revert AssetMismatch(data.baseToken, asset);
        }
        uint256 currentAmount = amount + data.internalContribution;

        for (uint256 i = 0; i < data.steps.length; ) {
            currentAmount = _executeSingleStep(data.steps[i], currentAmount);
            unchecked { ++i; }
        }

        uint256 amountOwed = amount + premium;
        _safeApprove(asset, aaveV3Pool, amountOwed);

        uint256 balanceCurrent = IERC20(asset).balanceOf(address(this));
        if (balanceCurrent < amountOwed) {
            revert InsufficientAtomicProfit(data.preLoanBalance, balanceCurrent, data.minNetProfit);
        }
        uint256 projectedFinalBalance = balanceCurrent - amountOwed;
        if (projectedFinalBalance < data.preLoanBalance + data.minNetProfit) {
            revert InsufficientAtomicProfit(data.preLoanBalance, projectedFinalBalance, data.minNetProfit);
        }

        uint256 netProfit = projectedFinalBalance - data.preLoanBalance;
        emit FlashArbitrageExecuted(asset, amount, premium, netProfit, block.timestamp);
        return true;
    }

    /// @inheritdoc IBalancerFlashLoanRecipient
    function receiveFlashLoan(
        address[] calldata tokens,
        uint256[] calldata amounts,
        uint256[] calldata feeAmounts,
        bytes calldata userData
    ) external override {
        if (!_inFlashLoan || msg.sender != balancerVault) {
            revert UnauthorizedFlashLoanCallback(msg.sender);
        }
        if (tokens.length == 0 || amounts.length != tokens.length || feeAmounts.length != tokens.length) {
            revert InvalidArrayLength();
        }

        FlashCallbackData memory data = abi.decode(userData, (FlashCallbackData));
        address asset = tokens[0];
        if (asset != data.baseToken) {
            revert AssetMismatch(data.baseToken, asset);
        }
        uint256 loanAmount = amounts[0];
        uint256 feeAmount = feeAmounts[0];

        uint256 currentAmount = loanAmount + data.internalContribution;
        for (uint256 i = 0; i < data.steps.length; ) {
            currentAmount = _executeSingleStep(data.steps[i], currentAmount);
            unchecked { ++i; }
        }

        uint256 amountOwed = loanAmount + feeAmount;
        _safeTransfer(asset, balancerVault, amountOwed);

        uint256 balanceAfterRepay = IERC20(asset).balanceOf(address(this));
        if (balanceAfterRepay < data.preLoanBalance + data.minNetProfit) {
            revert InsufficientAtomicProfit(data.preLoanBalance, balanceAfterRepay, data.minNetProfit);
        }

        uint256 netProfit = balanceAfterRepay - data.preLoanBalance;
        emit FlashArbitrageExecuted(asset, loanAmount, feeAmount, netProfit, block.timestamp);
    }

    // =========================================================================
    // Internal Step Execution
    // =========================================================================
    function _executeSingleStep(
        SwapStep memory step,
        uint256 amountIn
    ) internal returns (uint256 amountOut) {
        if (step.poolOrRouter == address(0) || step.tokenIn == address(0) || step.tokenOut == address(0)) {
            revert ZeroAddress();
        }
        _safeApprove(step.tokenIn, step.poolOrRouter, amountIn);

        uint256 outBefore = IERC20(step.tokenOut).balanceOf(address(this));

        if (step.dexType == DEX_UNIV2_VOLATILE) {
            address[] memory path = new address[](2);
            path[0] = step.tokenIn;
            path[1] = step.tokenOut;
            IUniswapV2Router(step.poolOrRouter).swapExactTokensForTokens(
                amountIn,
                0,
                path,
                address(this),
                block.timestamp
            );
        } else if (step.dexType == DEX_UNIV3) {
            IUniswapV3SwapRouter.ExactInputSingleParams memory params = IUniswapV3SwapRouter.ExactInputSingleParams({
                tokenIn: step.tokenIn,
                tokenOut: step.tokenOut,
                fee: step.feeTier,
                recipient: address(this),
                deadline: block.timestamp,
                amountIn: amountIn,
                amountOutMinimum: 0,
                sqrtPriceLimitX96: 0
            });
            IUniswapV3SwapRouter(step.poolOrRouter).exactInputSingle(params);
        } else if (step.dexType == DEX_CURVE_STABLE) {
            ICurvePool(step.poolOrRouter).exchange(
                step.curveIndexIn,
                step.curveIndexOut,
                amountIn,
                0
            );
        } else if (step.dexType == DEX_AERODROME_ROUTER) {
            address factory = IAerodromeRouter(step.poolOrRouter).defaultFactory();
            IAerodromeRouter.Route[] memory routes = new IAerodromeRouter.Route[](1);
            routes[0] = IAerodromeRouter.Route({
                from: step.tokenIn,
                to: step.tokenOut,
                stable: step.feeTier != 0,
                factory: factory
            });
            IAerodromeRouter(step.poolOrRouter).swapExactTokensForTokens(
                amountIn,
                0,
                routes,
                address(this),
                block.timestamp
            );
        } else if (step.dexType == DEX_UNIV3_ROUTER02) {
            ISwapRouter02.ExactInputSingleParams memory params = ISwapRouter02.ExactInputSingleParams({
                tokenIn: step.tokenIn,
                tokenOut: step.tokenOut,
                fee: step.feeTier,
                recipient: address(this),
                amountIn: amountIn,
                amountOutMinimum: 0,
                sqrtPriceLimitX96: 0
            });
            ISwapRouter02(step.poolOrRouter).exactInputSingle(params);
        } else {
            revert UnsupportedDexType(step.dexType);
        }

        uint256 outAfter = IERC20(step.tokenOut).balanceOf(address(this));
        amountOut = outAfter - outBefore;
    }

    // =========================================================================
    // Emergency Capital Rescue (Owner Only)
    // =========================================================================
    function withdrawToken(address token, uint256 amount) external onlyOwner {
        _safeTransfer(token, owner, amount);
        emit EmergencyWithdrawal(token, owner, amount);
    }

    function withdrawETH() external onlyOwner {
        uint256 bal = address(this).balance;
        (bool ok, ) = owner.call{value: bal}("");
        if (!ok) {
            revert EthTransferFailed(owner, bal);
        }
        emit EmergencyWithdrawal(address(0), owner, bal);
    }

    // =========================================================================
    // Low-Level Safe ERC20 Helpers
    // =========================================================================
    function _safeApprove(address token, address spender, uint256 amount) internal {
        uint256 currentAllowance = IERC20(token).allowance(address(this), spender);
        if (currentAllowance < amount) {
            (bool ok, bytes memory ret) = token.call(
                abi.encodeWithSelector(IERC20.approve.selector, spender, type(uint256).max)
            );
            if (!ok || (ret.length > 0 && !abi.decode(ret, (bool)))) {
                revert TokenTransferFailed(token, spender, amount);
            }
        }
    }

    function _safeTransfer(address token, address to, uint256 amount) internal {
        (bool ok, bytes memory ret) = token.call(
            abi.encodeWithSelector(IERC20.transfer.selector, to, amount)
        );
        if (!ok || (ret.length > 0 && !abi.decode(ret, (bool)))) {
            revert TokenTransferFailed(token, to, amount);
        }
    }

    receive() external payable {}
}
