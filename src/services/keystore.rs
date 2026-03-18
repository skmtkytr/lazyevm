use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::action::WalletEntry;

pub struct KeystoreService;

impl KeystoreService {
    /// List wallets: read keystore dir, resolve addresses via cast then keystore JSON
    pub async fn list_wallets() -> color_eyre::Result<Vec<WalletEntry>> {
        let keystore_dir = Self::keystore_dir();
        let mut names = Vec::new();

        let dir = match std::fs::read_dir(&keystore_dir) {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };

        for entry in dir.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.starts_with('.') {
                continue;
            }
            names.push(name);
        }

        names.sort();

        // Resolve addresses concurrently
        let mut handles = Vec::new();
        for name in &names {
            let name = name.clone();
            let dir = keystore_dir.clone();
            handles.push(tokio::spawn(async move {
                let address = Self::resolve_address(&name, &dir).await;
                WalletEntry {
                    name,
                    address: address.unwrap_or_default(),
                    balance: None,
                }
            }));
        }

        let mut wallets = Vec::new();
        for handle in handles {
            if let Ok(entry) = handle.await {
                wallets.push(entry);
            }
        }

        Ok(wallets)
    }

    /// Try to resolve address: cast wallet address (empty pw) → keystore JSON → empty
    async fn resolve_address(name: &str, keystore_dir: &Path) -> Option<String> {
        // 1. Try `cast wallet address --account <name> --password ""`
        if let Some(addr) = Self::try_cast_wallet_address(name, "").await {
            return Some(addr);
        }

        // 2. Try reading address from keystore JSON
        let path = keystore_dir.join(name);
        if let Some(addr) = Self::read_address_from_keystore(&path) {
            return Some(addr);
        }

        None
    }

    /// Try `cast wallet address` with a given password
    async fn try_cast_wallet_address(name: &str, password: &str) -> Option<String> {
        let output = Command::new("cast")
            .args([
                "wallet",
                "address",
                "--account",
                name,
                "--password",
                password,
            ])
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let addr = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if addr.starts_with("0x") && addr.len() >= 42 {
            Some(addr)
        } else {
            None
        }
    }

    /// Try to unlock a wallet with a specific password and return address
    pub async fn unlock_wallet(name: &str, password: &str) -> color_eyre::Result<String> {
        let output = Command::new("cast")
            .args([
                "wallet",
                "address",
                "--account",
                name,
                "--password",
                password,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to unlock: {}", err.trim()));
        }

        let addr = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(addr)
    }

    /// Read address from keystore JSON file (if present)
    fn read_address_from_keystore(path: &PathBuf) -> Option<String> {
        let content = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;

        let addr = json.get("address").and_then(|v| v.as_str())?;
        let addr = addr.trim();
        if addr.is_empty() {
            return None;
        }
        Some(if addr.starts_with("0x") {
            addr.to_string()
        } else {
            format!("0x{}", addr)
        })
    }

    fn keystore_dir() -> PathBuf {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".foundry").join("keystores")
    }

    /// Create a new wallet using `cast wallet new`
    pub async fn create_wallet() -> color_eyre::Result<String> {
        let output = Command::new("cast")
            .args(["wallet", "new"])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to create wallet: {}", err));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Fetch balance for an address using `cast balance`
    pub async fn get_balance(address: &str, rpc_url: &str) -> color_eyre::Result<String> {
        let output = Command::new("cast")
            .args(["balance", address, "--ether", "--rpc-url", rpc_url])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("{}", err.trim()));
        }

        let balance = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(format!("{} ETH", balance))
    }
}
