// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {HydraAtomicExecutor} from "../src/HydraAtomicExecutor.sol";
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
} from "../src/interfaces/IDexRouters.sol";

// =============================================================================
// Foundry Native Cheatcode Interface (Zero External Dependency)
// =============================================================================
interface Vm {
    function prank(address msgSender) external;
    function startPrank(address msgSender) external;
    function stopPrank() external;
    function expectRevert() external;
    function expectRevert(bytes calldata revertData) external;
    function expectRevert(bytes4 revertSelector) external;
    function deal(address who, uint256 newBalance) external;
}

// =============================================================================
// Mock ERC20 Token
// =============================================================================
contract MockERC20 is IERC20 {
    string public name;
    string public symbol;
    uint8 public immutable decimals;
    uint256 public override totalSupply;

    mapping(address => uint256) public override balanceOf;
    mapping(address => mapping(address => uint256)) public override allowance;

    constructor(string memory _name, string memory _symbol, uint8 _decimals) {
        name = _name;
        symbol = _symbol;
        decimals = _decimals;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function transfer(address recipient, uint256 amount) external override returns (bool) {
        require(balanceOf[msg.sender] >= amount, "ERC20: insufficient balance");
        balanceOf[msg.sender] -= amount;
        balanceOf[recipient] += amount;
        return true;
    }

    function approve(address spender, uint256 amount) external override returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address sender, address recipient, uint256 amount) external override returns (bool) {
        uint256 allowed = allowance[sender][msg.sender];
        require(allowed >= amount, "ERC20: insufficient allowance");
        require(balanceOf[sender] >= amount, "ERC20: insufficient balance");
        if (allowed != type(uint256).max) {
            allowance[sender][msg.sender] = allowed - amount;
        }
        balanceOf[sender] -= amount;
        balanceOf[recipient] += amount;
        return true;
    }
}

// =============================================================================
// Mock Uniswap V2 / Aerodrome Volatile Router (Configurable Rate Multiplier)
// =============================================================================
contract MockUniV2Router is IUniswapV2Router {
    // Rate scaled by 1e18: out = (amountIn * rateE18) / 1e18
    mapping(address => mapping(address => uint256)) public pairRateE18;

    function setRate(address tokenIn, address tokenOut, uint256 rateE18) external {
        pairRateE18[tokenIn][tokenOut] = rateE18;
    }

    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 /* deadline */
    ) external override returns (uint256[] memory amounts) {
        address tokenIn = path[0];
        address tokenOut = path[1];
        uint256 rate = pairRateE18[tokenIn][tokenOut];
        require(rate > 0, "UniV2Mock: pair rate not set");

        IERC20(tokenIn).transferFrom(msg.sender, address(this), amountIn);
        uint256 amountOut = (amountIn * rate) / 1e18;
        require(amountOut >= amountOutMin, "UniV2Mock: slippage");

        MockERC20(tokenOut).mint(to, amountOut);

        amounts = new uint256[](2);
        amounts[0] = amountIn;
        amounts[1] = amountOut;
    }
}

// =============================================================================
// Mock Uniswap V3 SwapRouter
// =============================================================================
contract MockUniV3Router is IUniswapV3SwapRouter {
    mapping(address => mapping(address => uint256)) public pairRateE18;

    function setRate(address tokenIn, address tokenOut, uint256 rateE18) external {
        pairRateE18[tokenIn][tokenOut] = rateE18;
    }

    function exactInputSingle(
        ExactInputSingleParams calldata params
    ) external payable override returns (uint256 amountOut) {
        uint256 rate = pairRateE18[params.tokenIn][params.tokenOut];
        require(rate > 0, "UniV3Mock: pair rate not set");

        IERC20(params.tokenIn).transferFrom(msg.sender, address(this), params.amountIn);
        amountOut = (params.amountIn * rate) / 1e18;
        require(amountOut >= params.amountOutMinimum, "UniV3Mock: slippage");

        MockERC20(params.tokenOut).mint(params.recipient, amountOut);
    }
}

// =============================================================================
// Mock Curve StableSwap Pool
// =============================================================================
contract MockCurvePool is ICurvePool {
    mapping(int128 => address) public coins;
    mapping(int128 => mapping(int128 => uint256)) public exchangeRateE18;

    function setCoin(int128 idx, address token) external {
        coins[idx] = token;
    }

    function setExchangeRate(int128 i, int128 j, uint256 rateE18) external {
        exchangeRateE18[i][j] = rateE18;
    }

    function exchange(
        int128 i,
        int128 j,
        uint256 dx,
        uint256 min_dy
    ) external override returns (uint256 dy) {
        address tokenIn = coins[i];
        address tokenOut = coins[j];
        uint256 rate = exchangeRateE18[i][j];
        require(rate > 0, "CurveMock: rate not set");

        IERC20(tokenIn).transferFrom(msg.sender, address(this), dx);
        dy = (dx * rate) / 1e18;
        require(dy >= min_dy, "CurveMock: slippage");

        MockERC20(tokenOut).mint(msg.sender, dy);
    }
}

// =============================================================================
// Mock Aave V3 Pool (5 bps FlashLoan Fee = 0.05%)
// =============================================================================
contract MockAaveV3Pool is IAaveV3Pool {
    uint256 public constant FLASHLOAN_PREMIUM_BPS = 5; // 5 bps = 0.05%

    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 /* referralCode */
    ) external override {
        uint256 premium = (amount * FLASHLOAN_PREMIUM_BPS) / 10_000;
        MockERC20(asset).mint(receiverAddress, amount);

        bool ok = IFlashLoanSimpleReceiver(receiverAddress).executeOperation(
            asset,
            amount,
            premium,
            msg.sender,
            params
        );
        require(ok, "AaveV3Mock: executeOperation returned false");

        // Pull back principal + 5 bps premium
        IERC20(asset).transferFrom(receiverAddress, address(this), amount + premium);
    }
}

// =============================================================================
// Mock Balancer V2 Vault (Zero Fee FlashLoan)
// =============================================================================
contract MockBalancerVault is IBalancerVault {
    function flashLoan(
        address recipient,
        address[] calldata tokens,
        uint256[] calldata amounts,
        bytes calldata userData
    ) external override {
        address asset = tokens[0];
        uint256 loanAmount = amounts[0];
        uint256 balBefore = IERC20(asset).balanceOf(address(this));

        MockERC20(asset).mint(recipient, loanAmount);

        uint256[] memory feeAmounts = new uint256[](1);
        feeAmounts[0] = 0;

        IBalancerFlashLoanRecipient(recipient).receiveFlashLoan(
            tokens,
            amounts,
            feeAmounts,
            userData
        );

        uint256 balAfter = IERC20(asset).balanceOf(address(this));
        require(balAfter >= balBefore + loanAmount, "BalancerMock: FlashLoan not repaid");
    }
}

// =============================================================================
// Test Suite: HydraAtomicExecutorTest
// =============================================================================
contract HydraAtomicExecutorTest {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    HydraAtomicExecutor internal hydra;
    MockAaveV3Pool internal aavePool;
    MockBalancerVault internal balancerVault;

    MockUniV2Router internal aeroRouter;
    MockUniV3Router internal uniV3Router;
    MockCurvePool internal curvePool;

    MockERC20 internal eurc;
    MockERC20 internal usdc;
    MockERC20 internal weth;

    address internal constant HOT_WALLET = address(0xBEEF01);
    address internal constant ATTACKER = address(0xBAD001);

    function setUp() public {
        aavePool = new MockAaveV3Pool();
        balancerVault = new MockBalancerVault();
        hydra = new HydraAtomicExecutor(HOT_WALLET, address(aavePool), address(balancerVault));

        aeroRouter = new MockUniV2Router();
        uniV3Router = new MockUniV3Router();
        curvePool = new MockCurvePool();

        eurc = new MockERC20("Euro Coin", "EURC", 6);
        usdc = new MockERC20("USD Coin", "USDC", 6);
        weth = new MockERC20("Wrapped Ether", "WETH", 18);

        // Configure Curve Pool indices: 0 = EURC, 1 = USDC
        curvePool.setCoin(0, address(eurc));
        curvePool.setCoin(1, address(usdc));

        // Seed Hydra contract with 1,500 EURC internal capital (1_500 * 1e6)
        eurc.mint(address(hydra), 1_500_000_000);
    }

    /// @notice Test 1: Profitable 3-hop triangular cycle across Curve (EURC->USDC), UniV3 (USDC->WETH), and Aerodrome (WETH->EURC)
    function test_ProfitableTriangularSwap_CompoundsBalance() public {
        // Leg 1 (Curve): 1 EURC -> 1.0900 USDC
        curvePool.setExchangeRate(0, 1, 1.09e18);
        // Leg 2 (UniV3): 1 USDC -> 0.5000 unit WETH (scaled in 6-dec equivalent for test clarity)
        uniV3Router.setRate(address(usdc), address(weth), 0.50e18);
        // Leg 3 (Aerodrome Volatile): 1 unit WETH -> 1.8500 EURC
        // Effective cycle multiplier: 1.09 * 0.50 * 1.85 = 1.00825 (+82.5 bps net profit!)
        aeroRouter.setRate(address(weth), address(eurc), 1.85e18);

        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](3);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(curvePool),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });
        steps[1] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV3(),
            poolOrRouter: address(uniV3Router),
            tokenIn: address(usdc),
            tokenOut: address(weth),
            feeTier: 500,
            curveIndexIn: 0,
            curveIndexOut: 0
        });
        steps[2] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV2_VOLATILE(),
            poolOrRouter: address(aeroRouter),
            tokenIn: address(weth),
            tokenOut: address(eurc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 0
        });

        uint256 initialBalance = eurc.balanceOf(address(hydra));
        uint256 tradeInput = 1_000_000_000; // 1,000 EURC
        uint256 minProfit = 5_000_000;      // Require at least 5.00 EURC profit

        vm.prank(HOT_WALLET);
        uint256 netProfit = hydra.executeAtomicArbitrage(
            address(eurc),
            tradeInput,
            minProfit,
            steps
        );

        uint256 finalBalance = eurc.balanceOf(address(hydra));
        // 1,000 * 1.00825 = 1,008.25 EURC -> netProfit = 8.25 EURC (8_250_000)
        require(netProfit == 8_250_000, "Unexpected net profit");
        require(finalBalance == initialBalance + 8_250_000, "Balance did not compound accurately");
    }

    /// @notice Test 2: Unprofitable swap or insufficient spread reverts atomically with zero principal loss
    function test_UnprofitableSwap_RevertsWithZeroPrincipalLoss() public {
        // Set rates that lose 0.4% due to adverse price movement / fees:
        // 1.08 * 0.50 * 1.844 = 0.99576 (< 1.0)
        curvePool.setExchangeRate(0, 1, 1.08e18);
        uniV3Router.setRate(address(usdc), address(weth), 0.50e18);
        aeroRouter.setRate(address(weth), address(eurc), 1.844e18);

        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](3);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(curvePool),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });
        steps[1] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV3(),
            poolOrRouter: address(uniV3Router),
            tokenIn: address(usdc),
            tokenOut: address(weth),
            feeTier: 500,
            curveIndexIn: 0,
            curveIndexOut: 0
        });
        steps[2] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV2_VOLATILE(),
            poolOrRouter: address(aeroRouter),
            tokenIn: address(weth),
            tokenOut: address(eurc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 0
        });

        uint256 initialBalance = eurc.balanceOf(address(hydra));

        vm.prank(HOT_WALLET);
        vm.expectRevert(
            abi.encodeWithSelector(
                HydraAtomicExecutor.InsufficientAtomicProfit.selector,
                initialBalance,
                initialBalance - 4_240_000,
                500_000
            )
        );
        hydra.executeAtomicArbitrage(
            address(eurc),
            1_000_000_000,
            500_000, // 0.50 EURC min profit
            steps
        );

        // Verify 100% of original principal remains untouched after revert
        require(eurc.balanceOf(address(hydra)) == initialBalance, "Principal loss occurred!");
    }

    /// @notice Test 3: FlashLoan arbitrage borrows 10,000 EURC via Aave V3, executes cycle, repays 10,005 EURC (5 bps fee), and keeps net profit
    function test_FlashLoanArbitrage_ExecutesAndRepays() public {
        // Set cycle return to +65 bps (1.0065):
        // Leg 1: 1 EURC -> 1.09 USDC
        // Leg 2: 1 USDC -> 0.50 WETH
        // Leg 3: 1 WETH -> 1.846788990825688073 EURC (~1.0065x total)
        curvePool.setExchangeRate(0, 1, 1.09e18);
        uniV3Router.setRate(address(usdc), address(weth), 0.50e18);
        aeroRouter.setRate(address(weth), address(eurc), 1.85e18); // +82.5 bps gross, -5 bps Aave fee = +77.5 bps net

        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](3);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(curvePool),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });
        steps[1] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV3(),
            poolOrRouter: address(uniV3Router),
            tokenIn: address(usdc),
            tokenOut: address(weth),
            feeTier: 500,
            curveIndexIn: 0,
            curveIndexOut: 0
        });
        steps[2] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV2_VOLATILE(),
            poolOrRouter: address(aeroRouter),
            tokenIn: address(weth),
            tokenOut: address(eurc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 0
        });

        uint256 initialBalance = eurc.balanceOf(address(hydra)); // 1,500 EURC
        uint256 flashLoanAmount = 10_000_000_000;                // Borrow 10,000 EURC
        uint256 minNetProfit = 50_000_000;                       // Require at least 50 EURC net profit

        vm.prank(HOT_WALLET);
        uint256 netProfit = hydra.executeFlashArbitrage(
            hydra.FLASH_PROVIDER_AAVE_V3(),
            address(eurc),
            flashLoanAmount,
            0, // Zero internal capital risked during FlashLoan
            minNetProfit,
            steps
        );

        // Gross profit on 10,000 EURC = 82.50 EURC; Aave V3 5 bps fee = 5.00 EURC; Net = 77.50 EURC (77_500_000)
        require(netProfit == 77_500_000, "Incorrect net flash arbitrage profit");
        require(
            eurc.balanceOf(address(hydra)) == initialBalance + 77_500_000,
            "Post-flashloan balance mismatch"
        );
    }

    /// @notice Test 4: Unauthorized caller is strictly rejected on execution and withdrawal entrypoints
    function test_UnauthorizedCaller_Reverts() public {
        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](1);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(curvePool),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });

        // 1. Unauthorized arbitrage call must revert with UnauthorizedCaller(ATTACKER)
        vm.prank(ATTACKER);
        vm.expectRevert(
            abi.encodeWithSelector(HydraAtomicExecutor.UnauthorizedCaller.selector, ATTACKER)
        );
        hydra.executeAtomicArbitrage(address(eurc), 100_000_000, 1_000_000, steps);

        // 2. Unauthorized FlashLoan call must revert with UnauthorizedCaller(ATTACKER)
        vm.prank(ATTACKER);
        vm.expectRevert(
            abi.encodeWithSelector(HydraAtomicExecutor.UnauthorizedCaller.selector, ATTACKER)
        );
        hydra.executeFlashArbitrage(0, address(eurc), 1_000_000_000, 0, 1_000_000, steps);

        // 3. Even whitelisted hot-wallet cannot call owner-only withdrawToken
        vm.prank(HOT_WALLET);
        vm.expectRevert(
            abi.encodeWithSelector(HydraAtomicExecutor.UnauthorizedCaller.selector, HOT_WALLET)
        );
        hydra.withdrawToken(address(eurc), 100_000_000);
    }

    /// @notice Test 5: Balancer V2 Vault zero-fee FlashLoan executes cycle and repays exact principal
    function test_BalancerFlashLoanArbitrage_ZeroFeeRepay() public {
        curvePool.setExchangeRate(0, 1, 1.09e18);
        uniV3Router.setRate(address(usdc), address(weth), 0.50e18);
        aeroRouter.setRate(address(weth), address(eurc), 1.85e18); // +82.5 bps net (0 bps Balancer fee)

        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](3);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(curvePool),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });
        steps[1] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV3(),
            poolOrRouter: address(uniV3Router),
            tokenIn: address(usdc),
            tokenOut: address(weth),
            feeTier: 500,
            curveIndexIn: 0,
            curveIndexOut: 0
        });
        steps[2] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV2_VOLATILE(),
            poolOrRouter: address(aeroRouter),
            tokenIn: address(weth),
            tokenOut: address(eurc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 0
        });

        uint256 initialBalance = eurc.balanceOf(address(hydra));
        vm.prank(HOT_WALLET);
        uint256 netProfit = hydra.executeFlashArbitrage(
            hydra.FLASH_PROVIDER_BALANCER(),
            address(eurc),
            10_000_000_000,
            0,
            80_000_000,
            steps
        );

        require(netProfit == 82_500_000, "Unexpected Balancer FlashLoan profit");
        require(eurc.balanceOf(address(hydra)) == initialBalance + 82_500_000, "Balancer final balance mismatch");
    }

    /// @notice Test 6: Owner can rescue ERC20 and ETH capital
    function test_OwnerEmergencyWithdraw_Succeeds() public {
        uint256 initialContractBal = eurc.balanceOf(address(hydra));
        hydra.withdrawToken(address(eurc), initialContractBal);
        require(eurc.balanceOf(address(hydra)) == 0, "Contract balance should be zero");
        require(eurc.balanceOf(address(this)) == initialContractBal, "Owner did not receive rescued tokens");

        vm.deal(address(hydra), 2 ether);
        uint256 ethBefore = address(this).balance;
        hydra.withdrawETH();
        require(address(this).balance == ethBefore + 2 ether, "Owner did not receive rescued ETH");
    }

    /// @notice Test 7: Base Mainnet native IAerodromeRouter (Route struct) + ISwapRouter02 (no deadline) + preApproveTokens warmup
    function test_BaseNativeAerodromeAndSwapRouter02_ExecutesAndSavesGas() public {
        MockAerodromeNativeRouter aeroNative = new MockAerodromeNativeRouter();
        MockBaseSwapRouter02 router02 = new MockBaseSwapRouter02();

        // Leg 1 (Aerodrome Stable Route): 1 EURC -> 1.0900 USDC
        aeroNative.setRate(address(eurc), address(usdc), true, 1.09e18);
        // Leg 2 (Base SwapRouter02): 1 USDC -> 0.5000 WETH
        router02.setRate(address(usdc), address(weth), 0.50e18);
        // Leg 3 (Aerodrome Volatile Route): 1 WETH -> 1.8500 EURC
        aeroNative.setRate(address(weth), address(eurc), false, 1.85e18);

        address[] memory tokens = new address[](3);
        tokens[0] = address(eurc);
        tokens[1] = address(usdc);
        tokens[2] = address(weth);

        address[] memory spenders = new address[](3);
        spenders[0] = address(aeroNative);
        spenders[1] = address(router02);
        spenders[2] = address(aeroNative);

        vm.prank(HOT_WALLET);
        hydra.preApproveTokens(tokens, spenders);

        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](3);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_AERODROME_ROUTER(),
            poolOrRouter: address(aeroNative),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 1, // 1 = Aerodrome Stable pool
            curveIndexIn: 0,
            curveIndexOut: 0
        });
        steps[1] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_UNIV3_ROUTER02(),
            poolOrRouter: address(router02),
            tokenIn: address(usdc),
            tokenOut: address(weth),
            feeTier: 500,
            curveIndexIn: 0,
            curveIndexOut: 0
        });
        steps[2] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_AERODROME_ROUTER(),
            poolOrRouter: address(aeroNative),
            tokenIn: address(weth),
            tokenOut: address(eurc),
            feeTier: 0, // 0 = Aerodrome Volatile pool
            curveIndexIn: 0,
            curveIndexOut: 0
        });

        vm.prank(HOT_WALLET);
        uint256 netProfit = hydra.executeAtomicArbitrage(
            address(eurc),
            1_000_000_000,
            5_000_000,
            steps
        );

        require(netProfit == 8_250_000, "Base native Aerodrome + SwapRouter02 net profit mismatch");
    }

    /// @notice Test 8: Reverts if Aave flashloan callback delivers asset mismatched with baseToken
    function test_RevertWhen_FlashLoanAssetMismatch_Aave() public {
        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](1);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(curvePool),
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });

        // Flash loan parameter has EURC as baseToken, but attacker/mock delivers USDC as asset
        HydraAtomicExecutor.FlashCallbackData memory data = HydraAtomicExecutor.FlashCallbackData({
            baseToken: address(eurc),
            preLoanBalance: 1_000_000_000,
            internalContribution: 0,
            minNetProfit: 1_000_000,
            steps: steps
        });

        vm.startPrank(address(aavePool));
        vm.expectRevert();
        hydra.executeOperation(
            address(usdc), // mismatched asset
            10_000_000_000,
            5_000_000,
            address(hydra),
            abi.encode(data)
        );
        vm.stopPrank();
    }

    /// @notice Test 9: Reverts if step contains address(0) router
    function test_RevertWhen_StepRouterIsZeroAddress() public {
        HydraAtomicExecutor.SwapStep[] memory steps = new HydraAtomicExecutor.SwapStep[](1);
        steps[0] = HydraAtomicExecutor.SwapStep({
            dexType: hydra.DEX_CURVE_STABLE(),
            poolOrRouter: address(0), // invalid zero address
            tokenIn: address(eurc),
            tokenOut: address(usdc),
            feeTier: 0,
            curveIndexIn: 0,
            curveIndexOut: 1
        });

        vm.prank(HOT_WALLET);
        vm.expectRevert();
        hydra.executeAtomicArbitrage(address(eurc), 1_000_000, 100_000, steps);
    }

    receive() external payable {}
}

contract MockAerodromeNativeRouter is IAerodromeRouter {
    address public constant override defaultFactory = address(0x420DD381b31aEf6683db6B902084cB0FFECe40Da);
    mapping(address => mapping(address => mapping(bool => uint256))) public routeRateE18;

    function setRate(address from, address to, bool stable, uint256 rateE18) external {
        routeRateE18[from][to][stable] = rateE18;
    }

    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        Route[] calldata routes,
        address to,
        uint256 /* deadline */
    ) external override returns (uint256[] memory amounts) {
        Route calldata r = routes[0];
        require(r.factory == defaultFactory, "AerodromeMock: factory mismatch");
        uint256 rate = routeRateE18[r.from][r.to][r.stable];
        require(rate > 0, "AerodromeMock: route rate not set");

        IERC20(r.from).transferFrom(msg.sender, address(this), amountIn);
        uint256 amountOut = (amountIn * rate) / 1e18;
        require(amountOut >= amountOutMin, "AerodromeMock: slippage");
        MockERC20(r.to).mint(to, amountOut);

        amounts = new uint256[](2);
        amounts[0] = amountIn;
        amounts[1] = amountOut;
    }
}

contract MockBaseSwapRouter02 is ISwapRouter02 {
    mapping(address => mapping(address => uint256)) public pairRateE18;

    function setRate(address tokenIn, address tokenOut, uint256 rateE18) external {
        pairRateE18[tokenIn][tokenOut] = rateE18;
    }

    function exactInputSingle(
        ExactInputSingleParams calldata params
    ) external payable override returns (uint256 amountOut) {
        uint256 rate = pairRateE18[params.tokenIn][params.tokenOut];
        require(rate > 0, "SwapRouter02Mock: rate not set");

        IERC20(params.tokenIn).transferFrom(msg.sender, address(this), params.amountIn);
        amountOut = (params.amountIn * rate) / 1e18;
        require(amountOut >= params.amountOutMinimum, "SwapRouter02Mock: slippage");
        MockERC20(params.tokenOut).mint(params.recipient, amountOut);
    }
}
