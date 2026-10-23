use hydra_core::NodeRole;
use std::env;
use std::fs;
use std::net::SocketAddr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    DryRun,
    Live,
}

impl ExecutionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DryRun => "DRY-RUN (Simulated Paper Execution)",
            Self::Live => "LIVE-DISABLED (Safe Dry-Run: Live order execution intentionally disabled)",
        }
    }
}

/// Complete runtime configuration for a Hydra-HFT v4.0 node.
#[derive(Clone, Debug)]
pub struct NodeConfig {
    pub role: NodeRole,
    pub mode: ExecutionMode,
    pub cpu_core_id: usize,
    pub initial_capital_eur: f64,
    pub min_net_profit_bps: f64,
    pub min_dex_profit_eur: f64,
    pub mesh_bind_addr: SocketAddr,
    pub mesh_peers: [Option<SocketAddr>; 2],
    pub mesh_mac_key: [u8; 32],
    pub max_ticks: usize,
    pub benchmark: bool,
    pub live_market: bool,
    pub paper_48h: bool,
    pub live_polls: usize,
    pub preflight_check: bool,
    pub max_gas_gwei: f64,
    pub base_rpc_url: String,
    pub wallet_address: Option<String>,
    pub executor_contract: Option<String>,
    pub has_private_key: bool,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            role: NodeRole::Tokyo,
            mode: ExecutionMode::DryRun,
            cpu_core_id: 2,
            initial_capital_eur: 150.0,
            min_net_profit_bps: 4.0,
            min_dex_profit_eur: 0.20,
            mesh_bind_addr: "127.0.0.1:0".parse().unwrap(),
            mesh_peers: [None, None],
            mesh_mac_key: *b"HYDRA_HFT_V4_MESH_MAC_SECRET_KEY",
            max_ticks: 10,
            benchmark: false,
            live_market: false,
            paper_48h: false,
            live_polls: 3,
            preflight_check: false,
            max_gas_gwei: 0.15,
            base_rpc_url: "https://base-rpc.publicnode.com".to_string(),
            wallet_address: None,
            executor_contract: None,
            has_private_key: false,
        }
    }
}

impl NodeConfig {
    /// Loads configuration safely:
    /// 1. Defaults
    /// 2. Key-value pairs from `.env` (without mutating process environment)
    /// 3. Overriding process environment variables
    /// 4. Overriding CLI flags
    pub fn from_env_and_args() -> Self {
        let mut cfg = Self::default();
        cfg.load_dotenv_safe();

        if let Ok(role_str) = env::var("HYDRA_NODE_ROLE") {
            cfg.role = Self::parse_role(&role_str);
        }
        if let Ok(mode_str) = env::var("HYDRA_EXECUTION_MODE") {
            if mode_str.eq_ignore_ascii_case("live") {
                cfg.mode = ExecutionMode::Live;
            }
        }
        if let Ok(cap_str) = env::var("HYDRA_INITIAL_CAPITAL_EUR") {
            if let Ok(val) = cap_str.parse::<f64>() {
                cfg.initial_capital_eur = val;
            }
        }
        if let Ok(bps_str) = env::var("HYDRA_MIN_NET_PROFIT_BPS") {
            if let Ok(val) = bps_str.parse::<f64>() {
                cfg.min_net_profit_bps = val;
            }
        }
        if let Ok(dex_str) = env::var("HYDRA_MIN_DEX_PROFIT_EUR") {
            if let Ok(val) = dex_str.parse::<f64>() {
                cfg.min_dex_profit_eur = val;
            }
        }
        if let Ok(rpc) = env::var("HYDRA_BASE_RPC_URL") {
            if !rpc.trim().is_empty() {
                cfg.base_rpc_url = rpc;
            }
        }
        if let Ok(gas) = env::var("HYDRA_MAX_GAS_GWEI") {
            if let Ok(val) = gas.parse::<f64>() {
                cfg.max_gas_gwei = val;
            }
        }
        if let Ok(addr) = env::var("HYDRA_WALLET_ADDRESS") {
            if !addr.trim().is_empty() {
                cfg.wallet_address = Some(addr);
            }
        }
        if let Ok(exec) = env::var("HYDRA_EXECUTOR_CONTRACT") {
            if !exec.trim().is_empty() {
                cfg.executor_contract = Some(exec);
            }
        }
        if let Ok(pk) = env::var("HYDRA_WALLET_PRIVATE_KEY") {
            cfg.has_private_key = !pk.trim().is_empty();
        }

        let args: Vec<String> = env::args().skip(1).collect();
        cfg.apply_args(&args);
        cfg
    }

    pub fn apply_args(&mut self, args: &[String]) {
        let mut i = 0usize;
        while i < args.len() {
            match args[i].as_str() {
                "--role" if i + 1 < args.len() => {
                    self.role = Self::parse_role(&args[i + 1]);
                    i += 2;
                }
                "--dry-run" => {
                    self.mode = ExecutionMode::DryRun;
                    i += 1;
                }
                "--live" => {
                    // Safe-by-default isolation: Live broadcasting remains disabled
                    self.mode = ExecutionMode::Live;
                    i += 1;
                }
                "--benchmark" => {
                    self.benchmark = true;
                    i += 1;
                }
                "--live-market" => {
                    self.live_market = true;
                    i += 1;
                }
                "--paper-48h" => {
                    self.paper_48h = true;
                    i += 1;
                }
                "--preflight-check" => {
                    self.preflight_check = true;
                    i += 1;
                }
                "--max-gas-gwei" if i + 1 < args.len() => {
                    if let Ok(g) = args[i + 1].parse::<f64>() {
                        self.max_gas_gwei = g;
                    }
                    i += 2;
                }
                "--rpc" if i + 1 < args.len() => {
                    self.base_rpc_url = args[i + 1].clone();
                    i += 2;
                }
                "--wallet" if i + 1 < args.len() => {
                    self.wallet_address = Some(args[i + 1].clone());
                    i += 2;
                }
                "--executor" if i + 1 < args.len() => {
                    self.executor_contract = Some(args[i + 1].clone());
                    i += 2;
                }
                "--min-dex-profit" if i + 1 < args.len() => {
                    if let Ok(d) = args[i + 1].parse::<f64>() {
                        self.min_dex_profit_eur = d;
                    }
                    i += 2;
                }
                "--min-bps" if i + 1 < args.len() => {
                    if let Ok(b) = args[i + 1].parse::<f64>() {
                        self.min_net_profit_bps = b;
                    }
                    i += 2;
                }
                "--polls" if i + 1 < args.len() => {
                    if let Ok(p) = args[i + 1].parse::<usize>() {
                        self.live_polls = p;
                    }
                    i += 2;
                }
                "--capital" if i + 1 < args.len() => {
                    if let Ok(c) = args[i + 1].parse::<f64>() {
                        self.initial_capital_eur = c;
                    }
                    i += 2;
                }
                "--ticks" if i + 1 < args.len() => {
                    if let Ok(t) = args[i + 1].parse::<usize>() {
                        self.max_ticks = t;
                    }
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }

    pub fn parse_role(s: &str) -> NodeRole {
        match s.trim().to_ascii_lowercase().as_str() {
            "tokyo" | "node-a" | "a" => NodeRole::Tokyo,
            "london" | "node-b" | "b" => NodeRole::London,
            "frankfurt" | "node-c" | "c" => NodeRole::Frankfurt,
            _ => NodeRole::Tokyo,
        }
    }

    fn apply_key_value(&mut self, key: &str, val: &str) {
        match key {
            "HYDRA_NODE_ROLE" => self.role = Self::parse_role(val),
            "HYDRA_EXECUTION_MODE" => {
                if val.eq_ignore_ascii_case("live") {
                    self.mode = ExecutionMode::Live;
                }
            }
            "HYDRA_INITIAL_CAPITAL_EUR" => {
                if let Ok(val) = val.parse::<f64>() {
                    self.initial_capital_eur = val;
                }
            }
            "HYDRA_MIN_NET_PROFIT_BPS" => {
                if let Ok(val) = val.parse::<f64>() {
                    self.min_net_profit_bps = val;
                }
            }
            "HYDRA_MIN_DEX_PROFIT_EUR" => {
                if let Ok(val) = val.parse::<f64>() {
                    self.min_dex_profit_eur = val;
                }
            }
            "HYDRA_BASE_RPC_URL" => {
                if !val.trim().is_empty() {
                    self.base_rpc_url = val.to_string();
                }
            }
            "HYDRA_MAX_GAS_GWEI" => {
                if let Ok(val) = val.parse::<f64>() {
                    self.max_gas_gwei = val;
                }
            }
            "HYDRA_WALLET_ADDRESS" => {
                if !val.trim().is_empty() {
                    self.wallet_address = Some(val.to_string());
                }
            }
            "HYDRA_EXECUTOR_CONTRACT" => {
                if !val.trim().is_empty() {
                    self.executor_contract = Some(val.to_string());
                }
            }
            "HYDRA_WALLET_PRIVATE_KEY" => {
                self.has_private_key = !val.trim().is_empty();
            }
            _ => {}
        }
    }

    fn load_dotenv_safe(&mut self) {
        let Ok(contents) = fs::read_to_string(".env") else {
            return;
        };
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = v.split('#').next().unwrap_or("").trim();
                self.apply_key_value(key, val);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_config_default_and_cli_precedence() {
        let mut cfg = NodeConfig::default();
        assert_eq!(cfg.mode, ExecutionMode::DryRun);
        assert_eq!(cfg.role, NodeRole::Tokyo);

        let args = vec![
            "--role".to_string(),
            "frankfurt".to_string(),
            "--capital".to_string(),
            "500.0".to_string(),
            "--live".to_string(),
        ];
        cfg.apply_args(&args);
        assert_eq!(cfg.role, NodeRole::Frankfurt);
        assert_eq!(cfg.initial_capital_eur, 500.0);
        assert_eq!(cfg.mode, ExecutionMode::Live);
        assert!(cfg.mode.as_str().contains("Safe Dry-Run"));
    }
}
