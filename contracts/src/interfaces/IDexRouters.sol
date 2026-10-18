// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title IERC20
/// @notice Minimal ERC-20 interface for zero-overhead balance checks, approvals, and transfers.
interface IERC20 {
    function totalSupply() external view returns (uint256);
    function balanceOf(address account) external view returns (uint256);
    function transfer(address recipient, uint256 amount) external returns (bool);
    function allowance(address owner, address spender) external view returns (uint256);
    function approve(address spender, uint256 amount) external returns (bool);
    function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
}

/// @title IUniswapV2Router
/// @notice Interface for Uniswap V2 / Camelot V2 constant-product routers.
interface IUniswapV2Router {
    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        address[] calldata path,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

/// @title IAerodromeRouter
/// @notice Native interface for Aerodrome Finance Router on Base Mainnet (`0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43`).
interface IAerodromeRouter {
    struct Route {
        address from;
        address to;
        bool stable;
        address factory;
    }

    function defaultFactory() external view returns (address);

    function swapExactTokensForTokens(
        uint256 amountIn,
        uint256 amountOutMin,
        Route[] calldata routes,
        address to,
        uint256 deadline
    ) external returns (uint256[] memory amounts);
}

/// @title IUniswapV3SwapRouter
/// @notice Interface for Uniswap V3 SwapRouter V1 (with `deadline` in struct, e.g. Arbitrum One `0xE592427A0AEce92De3Edee1F18E0157C05861564`).
interface IUniswapV3SwapRouter {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 deadline;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
}

/// @title ISwapRouter02
/// @notice Interface for Uniswap V3 SwapRouter02 (without `deadline` in struct, deployed on Base Mainnet `0x2626664c2603336E57B271c5C0b26F421741e481`).
interface ISwapRouter02 {
    struct ExactInputSingleParams {
        address tokenIn;
        address tokenOut;
        uint24 fee;
        address recipient;
        uint256 amountIn;
        uint256 amountOutMinimum;
        uint160 sqrtPriceLimitX96;
    }

    function exactInputSingle(ExactInputSingleParams calldata params) external payable returns (uint256 amountOut);
}

/// @title ICurvePool
/// @notice Interface for Curve Finance StableSwap / CryptoSwap pools.
interface ICurvePool {
    function exchange(
        int128 i,
        int128 j,
        uint256 dx,
        uint256 min_dy
    ) external returns (uint256 dy);
}

/// @title IAaveV3Pool
/// @notice Interface for Aave V3 Pool FlashLoanSimple execution.
interface IAaveV3Pool {
    function flashLoanSimple(
        address receiverAddress,
        address asset,
        uint256 amount,
        bytes calldata params,
        uint16 referralCode
    ) external;
}

/// @title IFlashLoanSimpleReceiver
/// @notice Callback interface invoked by Aave V3 Pool during `flashLoanSimple`.
interface IFlashLoanSimpleReceiver {
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 premium,
        address initiator,
        bytes calldata params
    ) external returns (bool);
}

/// @title IBalancerVault
/// @notice Interface for Balancer V2 Vault zero-fee FlashLoans on Base / Arbitrum (`0xBA12222222228d8Ba445958a75a0704d566BF2C8`).
interface IBalancerVault {
    function flashLoan(
        address recipient,
        address[] calldata tokens,
        uint256[] calldata amounts,
        bytes calldata userData
    ) external;
}

/// @title IBalancerFlashLoanRecipient
/// @notice Callback interface invoked by Balancer V2 Vault during `flashLoan`.
interface IBalancerFlashLoanRecipient {
    function receiveFlashLoan(
        address[] calldata tokens,
        uint256[] calldata amounts,
        uint256[] calldata feeAmounts,
        bytes calldata userData
    ) external;
}
