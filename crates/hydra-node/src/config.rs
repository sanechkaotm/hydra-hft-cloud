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
            Self::DryRun => "DRY-RUN (Paper-Trading Simulator)",
            Self::Live => "LIVE (Production Atomic/IOC Execution)",
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
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            role: NodeRole::Tokyo,
            mode: ExecutionMode::DryRun,
            cpu_core_id: 2,
            initial_capital_eur: 150.0,
            min_net_profit_bps: 4.0,
            min_dex_profit_eur: 0.05,
            mesh_bind_addr: "127.0.0.1:0".parse().unwrap(),
            mesh_peers: [None, None],
            mesh_mac_key: *b"HYDRA_HFT_V4_MESH_MAC_SECRET_KEY",
            max_ticks: 10,
            benchmark: false,
            live_market: false,
            paper_48h: false,
            live_polls: 3,
        }
    }
}

impl NodeConfig {
    /// Loads configuration from `.env` (if present), environment variables, and CLI flags:
    /// `--role tokyo|london|frankfurt`, `--dry-run`, `--live`, `--capital <EUR>`, `--ticks <N>`, `--benchmark`, `--live-market`, `--paper-48h`, `--polls <N>`.
    pub fn from_env_and_args() -> Self {
        let mut cfg = Self::default();
        Self::load_dotenv_into_process();

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

        let args: Vec<String> = env::args().skip(1).collect();
        let mut i = 0usize;
        while i < args.len() {
            match args[i].as_str() {
                "--role" if i + 1 < args.len() => {
                    cfg.role = Self::parse_role(&args[i + 1]);
                    i += 2;
                }
                "--dry-run" => {
                    cfg.mode = ExecutionMode::DryRun;
                    i += 1;
                }
                "--live" => {
                    cfg.mode = ExecutionMode::Live;
                    i += 1;
                }
                "--benchmark" => {
                    cfg.benchmark = true;
                    i += 1;
                }
                "--live-market" => {
                    cfg.live_market = true;
                    i += 1;
                }
                "--paper-48h" => {
                    cfg.paper_48h = true;
                    i += 1;
                }
                "--polls" if i + 1 < args.len() => {
                    if let Ok(p) = args[i + 1].parse::<usize>() {
                        cfg.live_polls = p;
                    }
                    i += 2;
                }
                "--capital" if i + 1 < args.len() => {
                    if let Ok(c) = args[i + 1].parse::<f64>() {
                        cfg.initial_capital_eur = c;
                    }
                    i += 2;
                }
                "--ticks" if i + 1 < args.len() => {
                    if let Ok(t) = args[i + 1].parse::<usize>() {
                        cfg.max_ticks = t;
                    }
                    i += 2;
                }
                _ => {
                    i += 1;
                }
            }
        }

        cfg
    }

    pub fn parse_role(s: &str) -> NodeRole {
        match s.trim().to_ascii_lowercase().as_str() {
            "tokyo" | "node-a" | "a" => NodeRole::Tokyo,
            "london" | "node-b" | "b" => NodeRole::London,
            "frankfurt" | "node-c" | "c" => NodeRole::Frankfurt,
            _ => NodeRole::Tokyo,
        }
    }

    fn load_dotenv_into_process() {
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
                if env::var_os(key).is_none() {
                    unsafe {
                        env::set_var(key, val);
                    }
                }
            }
        }
    }
}
