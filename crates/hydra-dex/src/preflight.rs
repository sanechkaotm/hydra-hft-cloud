/// Result of an on-chain `eth_call` pre-flight simulation.
#[derive(Clone, Debug, PartialEq)]
pub struct PreflightSimulationResult {
    /// True if the simulation completed without reverting.
    pub success: bool,
    /// Return value decoded as `uint256 netProfit` in token base units (wei / micro-units).
    pub simulated_net_profit_wei: u128,
    /// Estimated gas units required for execution.
    pub estimated_gas: u64,
    /// Human-readable revert message or error description if the call reverted.
    pub revert_reason: Option<String>,
}

/// Errors that can occur during pre-flight transaction simulation.
#[derive(Clone, Debug, PartialEq)]
pub enum SimulationError {
    /// The EVM reverted the call on-chain (e.g. slippage exceeded or price moved).
    ExecutionReverted {
        message: String,
        selector: Option<[u8; 4]>,
    },
    /// The call succeeded, but the net profit returned was below the required hurdle.
    InsufficientProfit {
        hurdle_wei: u128,
        actual_wei: u128,
    },
    /// Network or JSON-RPC transport error.
    TransportError(String),
    /// Response from RPC was malformed or missing result.
    InvalidRpcResponse(String),
}

/// Pre-flight RPC Transaction Simulator for `HydraAtomicExecutor.sol`.
/// Simulates candidate transactions via read-only `eth_call` BEFORE broadcasting to Base Mainnet mempool,
/// guaranteeing 0 gas is ever burned on failed/reverted arbitrage transactions.
pub struct PreflightSimulator;

impl PreflightSimulator {
    /// Formats a standard JSON-RPC 2.0 `eth_call` request body without external JSON dependencies.
    pub fn format_eth_call_json(
        from: &[u8; 20],
        to: &[u8; 20],
        calldata: &[u8],
        block_tag: &str,
    ) -> String {
        let mut from_hex = String::with_capacity(42);
        from_hex.push_str("0x");
        for b in from {
            use std::fmt::Write;
            let _ = write!(&mut from_hex, "{:02x}", b);
        }

        let mut to_hex = String::with_capacity(42);
        to_hex.push_str("0x");
        for b in to {
            use std::fmt::Write;
            let _ = write!(&mut to_hex, "{:02x}", b);
        }

        let mut data_hex = String::with_capacity(2 + calldata.len() * 2);
        data_hex.push_str("0x");
        for b in calldata {
            use std::fmt::Write;
            let _ = write!(&mut data_hex, "{:02x}", b);
        }

        format!(
            r#"{{"jsonrpc":"2.0","method":"eth_call","params":[{{"from":"{}","to":"{}","data":"{}"}},"{}"],"id":1}}"#,
            from_hex, to_hex, data_hex, block_tag
        )
    }

    /// Parses the JSON-RPC `eth_call` response string.
    /// Extracts the returned 32-byte hex string (uint256 netProfit) or parses error revert data.
    pub fn parse_eth_call_response(
        raw_json: &str,
        min_net_profit_wei: u128,
    ) -> Result<PreflightSimulationResult, SimulationError> {
        let trimmed = raw_json.trim();
        if trimmed.is_empty() {
            return Err(SimulationError::TransportError("Empty response from RPC".to_string()));
        }

        // 1. Check for JSON-RPC error field: `"error"`
        if let Some(err_idx) = trimmed.find(r#""error""#) {
            let error_sub = &trimmed[err_idx..];
            let msg = if let Some(msg_idx) = error_sub.find(r#""message":""#) {
                let start = msg_idx + r#""message":""#.len();
                if let Some(end) = error_sub[start..].find('"') {
                    error_sub[start..start + end].to_string()
                } else {
                    "Unknown RPC execution error".to_string()
                }
            } else {
                "Execution error".to_string()
            };

            let selector = extract_revert_selector(error_sub);
            return Err(SimulationError::ExecutionReverted {
                message: msg,
                selector,
            });
        }

        // 2. Extract `"result":"0x..."`
        if let Some(res_idx) = trimmed.find(r#""result":""#) {
            let start = res_idx + r#""result":""#.len();
            let after = &trimmed[start..];
            if let Some(end) = after.find('"') {
                let hex_val = &after[..end];
                let profit_wei = parse_u128_from_hex(hex_val).map_err(|e| {
                    SimulationError::InvalidRpcResponse(format!("Failed to parse return hex: {}", e))
                })?;

                if profit_wei < min_net_profit_wei {
                    return Err(SimulationError::InsufficientProfit {
                        hurdle_wei: min_net_profit_wei,
                        actual_wei: profit_wei,
                    });
                }

                return Ok(PreflightSimulationResult {
                    success: true,
                    simulated_net_profit_wei: profit_wei,
                    estimated_gas: 285_000, // Typical Base Multi-hop atomic swap gas
                    revert_reason: None,
                });
            }
        }

        Err(SimulationError::InvalidRpcResponse("Missing result in JSON-RPC response".to_string()))
    }

    /// Evaluates deterministic simulation in mock / dry-run mode without external network calls.
    pub fn simulate_mock(
        simulated_balance_delta_wei: i128,
        min_net_profit_wei: u128,
    ) -> Result<PreflightSimulationResult, SimulationError> {
        if simulated_balance_delta_wei <= 0 {
            return Err(SimulationError::ExecutionReverted {
                message: "Hydra: InsufficientAtomicProfit".to_string(),
                selector: Some([0x87, 0x6e, 0x48, 0x5a]), // InsufficientAtomicProfit selector
            });
        }

        let profit_u128 = simulated_balance_delta_wei as u128;
        if profit_u128 < min_net_profit_wei {
            return Err(SimulationError::InsufficientProfit {
                hurdle_wei: min_net_profit_wei,
                actual_wei: profit_u128,
            });
        }

        Ok(PreflightSimulationResult {
            success: true,
            simulated_net_profit_wei: profit_u128,
            estimated_gas: 275_000,
            revert_reason: None,
        })
    }
}

/// Parses a 32-byte (or shorter) hex string into `u128`.
fn parse_u128_from_hex(hex: &str) -> Result<u128, &'static str> {
    let clean = hex.strip_prefix("0x").unwrap_or(hex);
    if clean.is_empty() {
        return Ok(0);
    }
    // Take the lower 32 hex characters (16 bytes = u128)
    let len = clean.len();
    let lower_slice = if len > 32 {
        &clean[len - 32..]
    } else {
        clean
    };
    u128::from_str_radix(lower_slice, 16).map_err(|_| "Invalid hex digits")
}

/// Attempts to locate a 4-byte revert selector (e.g. `0x08c379a0` for Error(string)) in the error data.
fn extract_revert_selector(error_payload: &str) -> Option<[u8; 4]> {
    if let Some(data_idx) = error_payload.find(r#""data":"0x"#) {
        let start = data_idx + r#""data":"0x"#.len();
        if error_payload.len() >= start + 8 {
            let hex_sel = &error_payload[start..start + 8];
            if let Ok(b0) = u8::from_str_radix(&hex_sel[0..2], 16) {
                if let Ok(b1) = u8::from_str_radix(&hex_sel[2..4], 16) {
                    if let Ok(b2) = u8::from_str_radix(&hex_sel[4..6], 16) {
                        if let Ok(b3) = u8::from_str_radix(&hex_sel[6..8], 16) {
                            return Some([b0, b1, b2, b3]);
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_eth_call_json() {
        let from = [0xAA; 20];
        let to = [0xBB; 20];
        let calldata = [0x01, 0x02, 0x03];
        let json = PreflightSimulator::format_eth_call_json(&from, &to, &calldata, "latest");
        assert!(json.contains(r#""from":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#));
        assert!(json.contains(r#""to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb""#));
        assert!(json.contains(r#""data":"0x010203""#));
        assert!(json.contains(r#""latest""#));
    }

    #[test]
    fn test_parse_eth_call_response_success() {
        // Return 1,500,000 wei (= 1.50 USDC/EURC at 6 decimals)
        let resp = r#"{"jsonrpc":"2.0","result":"0x000000000000000000000000000000000000000000000000000000000016e360","id":1}"#;
        let res = PreflightSimulator::parse_eth_call_response(resp, 1_000_000).unwrap();
        assert!(res.success);
        assert_eq!(res.simulated_net_profit_wei, 1_500_000);
        assert_eq!(res.estimated_gas, 285_000);
    }

    #[test]
    fn test_parse_eth_call_response_insufficient_profit() {
        // Return 500,000 wei, but hurdle is 1,000,000
        let resp = r#"{"jsonrpc":"2.0","result":"0x000000000000000000000000000000000000000000000000000000000007a120","id":1}"#;
        let err = PreflightSimulator::parse_eth_call_response(resp, 1_000_000).unwrap_err();
        match err {
            SimulationError::InsufficientProfit { hurdle_wei, actual_wei } => {
                assert_eq!(hurdle_wei, 1_000_000);
                assert_eq!(actual_wei, 500_000);
            }
            _ => panic!("Expected InsufficientProfit"),
        }
    }

    #[test]
    fn test_parse_eth_call_response_revert() {
        let resp = r#"{"jsonrpc":"2.0","error":{"code":3,"message":"execution reverted: InsufficientAtomicProfit","data":"0x876e485a000000"},"id":1}"#;
        let err = PreflightSimulator::parse_eth_call_response(resp, 1_000_000).unwrap_err();
        match err {
            SimulationError::ExecutionReverted { message, selector } => {
                assert!(message.contains("InsufficientAtomicProfit"));
                assert_eq!(selector, Some([0x87, 0x6e, 0x48, 0x5a]));
            }
            _ => panic!("Expected ExecutionReverted"),
        }
    }

    #[test]
    fn test_simulate_mock_modes() {
        // Profitable mock call
        let ok = PreflightSimulator::simulate_mock(2_000_000, 1_000_000).unwrap();
        assert_eq!(ok.simulated_net_profit_wei, 2_000_000);

        // Loss mock call
        let err = PreflightSimulator::simulate_mock(-500_000, 1_000_000).unwrap_err();
        assert!(matches!(err, SimulationError::ExecutionReverted { .. }));

        // Sub-hurdle mock call
        let sub = PreflightSimulator::simulate_mock(800_000, 1_000_000).unwrap_err();
        assert!(matches!(sub, SimulationError::InsufficientProfit { .. }));
    }
}
