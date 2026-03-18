use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub networks: NetworkConfig,
    #[serde(default)]
    pub tokens: TokenConfig,
    #[serde(default)]
    pub anvil: AnvilConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub active: String,
    pub list: Vec<Network>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    pub list: Vec<TokenEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenEntry {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub balance_slot: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnvilConfig {
    pub fork_network: Option<String>,
    #[serde(default)]
    pub fork_url: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }

        // Check ETH_RPC_URL env var and create default config
        let mut config = Self::default();
        if let Ok(rpc) = std::env::var("ETH_RPC_URL") {
            // Add as first entry and set active
            let name = "ENV".to_string();
            config.networks.list.insert(
                0,
                Network {
                    name: name.clone(),
                    url: rpc,
                },
            );
            config.networks.active = name;
        }
        let _ = config.save();
        config
    }

    pub fn save(&self) -> color_eyre::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".config").join("lazyevm").join("config.toml")
    }

    pub fn active_network(&self) -> Option<&Network> {
        self.networks
            .list
            .iter()
            .find(|n| n.name == self.networks.active)
    }

    pub fn active_rpc_url(&self) -> &str {
        self.active_network()
            .map(|n| n.url.as_str())
            .unwrap_or("http://localhost:8545")
    }

    pub fn active_network_name(&self) -> &str {
        self.active_network()
            .map(|n| n.name.as_str())
            .unwrap_or("unknown")
    }

    pub fn select_network(&mut self, name: &str) {
        if self.networks.list.iter().any(|n| n.name == name) {
            self.networks.active = name.to_string();
            let _ = self.save();
        }
    }

    pub fn add_network(&mut self, name: String, url: String) {
        // Remove existing with same name
        self.networks.list.retain(|n| n.name != name);
        self.networks.list.push(Network { name, url });
        let _ = self.save();
    }

    pub fn remove_network(&mut self, name: &str) {
        self.networks.list.retain(|n| n.name != name);
        // If we removed the active one, switch to first available
        if self.networks.active == name {
            self.networks.active = self
                .networks
                .list
                .first()
                .map(|n| n.name.clone())
                .unwrap_or_default();
        }
        let _ = self.save();
    }

    /// Resolve the fork RPC URL: direct fork_url takes priority, then fork_network name lookup
    pub fn fork_rpc_url(&self) -> Option<&str> {
        // Direct URL override
        if let Some(ref url) = self.anvil.fork_url {
            if !url.is_empty() {
                return Some(url.as_str());
            }
        }
        // Fallback: resolve from network name
        let fork_name = self
            .anvil
            .fork_network
            .as_deref()
            .unwrap_or("Ethereum Mainnet");
        self.networks
            .list
            .iter()
            .find(|n| n.name == fork_name)
            .map(|n| n.url.as_str())
    }

    /// Set the fork URL directly and save config
    pub fn set_fork_url(&mut self, url: String) {
        if url.is_empty() {
            self.anvil.fork_url = None;
        } else {
            self.anvil.fork_url = Some(url);
        }
        let _ = self.save();
    }

    pub fn add_token(&mut self, token: TokenEntry) {
        self.tokens.list.retain(|t| t.address != token.address);
        self.tokens.list.push(token);
        let _ = self.save();
    }

    pub fn remove_token(&mut self, address: &str) {
        self.tokens.list.retain(|t| t.address != address);
        let _ = self.save();
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            networks: NetworkConfig {
                active: "Anvil Local".to_string(),
                list: vec![
                    Network {
                        name: "Anvil Local".to_string(),
                        url: "http://localhost:8545".to_string(),
                    },
                    Network {
                        name: "Ethereum Mainnet".to_string(),
                        url: "https://eth.llamarpc.com".to_string(),
                    },
                    Network {
                        name: "Sepolia".to_string(),
                        url: "https://rpc.sepolia.org".to_string(),
                    },
                ],
            },
            tokens: TokenConfig::default(),
            anvil: AnvilConfig::default(),
        }
    }
}

impl Default for TokenConfig {
    fn default() -> Self {
        TokenConfig {
            list: vec![
                TokenEntry {
                    address: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                    symbol: "USDC".to_string(),
                    name: "USD Coin".to_string(),
                    decimals: 6,
                    balance_slot: Some(9),
                },
                TokenEntry {
                    address: "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
                    symbol: "USDT".to_string(),
                    name: "Tether USD".to_string(),
                    decimals: 6,
                    balance_slot: Some(2),
                },
                TokenEntry {
                    address: "0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string(),
                    symbol: "DAI".to_string(),
                    name: "Dai Stablecoin".to_string(),
                    decimals: 18,
                    balance_slot: Some(2),
                },
                TokenEntry {
                    address: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                    symbol: "WETH".to_string(),
                    name: "Wrapped Ether".to_string(),
                    decimals: 18,
                    balance_slot: Some(3),
                },
            ],
        }
    }
}

impl Default for AnvilConfig {
    fn default() -> Self {
        AnvilConfig {
            fork_network: Some("Ethereum Mainnet".to_string()),
            fork_url: None,
        }
    }
}
