// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {HydraAtomicExecutor} from "../src/HydraAtomicExecutor.sol";

interface VmScript {
    function envUint(string calldata name) external returns (uint256);
    function envAddress(string calldata name) external returns (address);
    function startBroadcast(uint256 privateKey) external;
    function stopBroadcast() external;
}

/// @title DeployHydraScript
/// @notice Production deployment & router allowance warmup script for Base Mainnet (8453) and Arbitrum One (42161).
contract DeployHydraScript {
    VmScript internal constant vm = VmScript(address(uint160(uint256(keccak256("hevm cheat code")))));

    // =========================================================================
    // Base Mainnet (Chain ID: 8453) Verified Production Addresses
    // =========================================================================
    address public constant BASE_AAVE_V3_POOL     = 0xA238Dd80C259a72e81d7e4664a9801593F98d1c5;
    address public constant BASE_BALANCER_VAULT   = 0xBA12222222228d8Ba445958a75a0704d566BF2C8;
    address public constant BASE_AERODROME_ROUTER = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43;
    address public constant BASE_UNIV3_ROUTER02   = 0x2626664c2603336E57B271c5C0b26F421741e481;

    address public constant BASE_USDC = 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913;
    address public constant BASE_EURC = 0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42;
    address public constant BASE_WETH = 0x4200000000000000000000000000000000000006;
    address public constant BASE_DAI  = 0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb;
    address public constant BASE_AERO = 0x940181a94A35A4569E4529A3CDfB74e38FD98631;

    // =========================================================================
    // Arbitrum One (Chain ID: 42161) Verified Production Addresses
    // =========================================================================
    address public constant ARB_AAVE_V3_POOL   = 0x794a61358D6845594F94dc1DB02A252b5b4814aD;
    address public constant ARB_BALANCER_VAULT = 0xBA12222222228d8Ba445958a75a0704d566BF2C8;
    address public constant ARB_UNIV3_ROUTER   = 0xE592427A0AEce92De3Edee1F18E0157C05861564;

    address public constant ARB_USDC = 0xaf88d065e77c8cC2239327C5EDb3A432268e5831;
    address public constant ARB_USDT = 0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9;
    address public constant ARB_WETH = 0x82aF49447D8a07e3bd95BD0d56f35241523fBab1;
    address public constant ARB_ARB  = 0x912CE59144191C1204E64559FE8253a0e49E6548;

    function run() external returns (HydraAtomicExecutor deployed) {
        uint256 deployerPrivKey = vm.envUint("HYDRA_HOT_WALLET_PRIVKEY");
        address hotWalletExecutor = vm.envAddress("HYDRA_EXECUTOR_ADDRESS");

        vm.startBroadcast(deployerPrivKey);

        if (block.chainid == 8453) {
            deployed = new HydraAtomicExecutor(
                hotWalletExecutor,
                BASE_AAVE_V3_POOL,
                BASE_BALANCER_VAULT
            );
        } else if (block.chainid == 42161) {
            deployed = new HydraAtomicExecutor(
                hotWalletExecutor,
                ARB_AAVE_V3_POOL,
                ARB_BALANCER_VAULT
            );
        } else {
            deployed = new HydraAtomicExecutor(
                hotWalletExecutor,
                BASE_AAVE_V3_POOL,
                BASE_BALANCER_VAULT
            );
        }

        vm.stopBroadcast();
    }
}
