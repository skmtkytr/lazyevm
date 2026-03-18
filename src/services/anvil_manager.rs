use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, AnvilAccount};

pub struct AnvilManager {
    child: Option<Child>,
    port: u16,
    pub fork_url: Option<String>,
}

impl AnvilManager {
    pub fn new() -> Self {
        Self {
            child: None,
            port: 8545,
            fork_url: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// Check if the child process has exited and clean up if so.
    /// This prevents the "already running" error when the process died unexpectedly.
    pub fn cleanup_if_exited(&mut self) {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    // Process has exited — clean up
                    self.child = None;
                    self.fork_url = None;
                }
                Ok(None) => {
                    // Still running
                }
                Err(_) => {
                    // Error checking status — assume dead
                    self.child = None;
                    self.fork_url = None;
                }
            }
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Path to the persisted anvil state file
    pub fn state_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("lazyevm");
        config_dir.join("anvil-state.json")
    }

    /// Start anvil and stream logs back through the action channel
    pub async fn start(
        &mut self,
        port: u16,
        action_tx: UnboundedSender<Action>,
    ) -> color_eyre::Result<()> {
        self.cleanup_if_exited();
        if self.child.is_some() {
            return Err(eyre::eyre!("Anvil is already running"));
        }

        self.port = port;

        let mut cmd = Command::new("anvil");
        cmd.arg("--port").arg(port.to_string());

        // Restore previous state if available
        let state_path = Self::state_path();
        if state_path.exists() {
            cmd.arg("--load-state").arg(&state_path);
        }

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");

        self.child = Some(child);

        // Parse initial output for accounts
        let tx_stdout = action_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut accounts: Vec<String> = Vec::new();
            let mut parsing_accounts = false;
            let mut parsing_keys = false;
            let mut current_keys: Vec<String> = Vec::new();
            let mut started = false;

            while let Ok(Some(line)) = lines.next_line().await {
                // Parse anvil startup output
                if line.contains("Available Accounts") {
                    parsing_accounts = true;
                    parsing_keys = false;
                    continue;
                }
                if line.contains("Private Keys") {
                    parsing_accounts = false;
                    parsing_keys = true;
                    continue;
                }
                if line.contains("Wallet") || line.contains("Base Fee") || line.contains("Listening") {
                    parsing_accounts = false;
                    parsing_keys = false;

                    if !accounts.is_empty() {
                        // Merge keys with accounts
                        let mut full_accounts: Vec<AnvilAccount> = Vec::new();
                        for (i, addr) in accounts.iter().enumerate() {
                            full_accounts.push(AnvilAccount {
                                address: addr.clone(),
                                key: current_keys.get(i).cloned().unwrap_or_default(),
                                balance: "10000 ETH".to_string(),
                            });
                        }
                        let _ = tx_stdout.send(Action::AnvilAccounts(full_accounts));
                        accounts.clear();
                        current_keys.clear();
                    }
                }

                if line.contains("Listening on") {
                    started = true;
                    let _ = tx_stdout.send(Action::AnvilStarted { port });
                }

                if parsing_accounts {
                    // Lines like: (0) 0x1234...5678 (10000.000000000000000000 ETH)
                    if let Some(addr) = parse_anvil_account(&line) {
                        accounts.push(addr);
                    }
                }

                if parsing_keys {
                    // Lines like: (0) 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
                    if let Some(key) = parse_anvil_key(&line) {
                        current_keys.push(key);
                    }
                }

                let _ = tx_stdout.send(Action::AnvilLog(line));
            }

            // stdout EOF — process exited
            if !started {
                let _ = tx_stdout.send(Action::AnvilError("Anvil process exited unexpectedly".to_string()));
            }
            let _ = tx_stdout.send(Action::AnvilStopped);
        });

        // Stream stderr
        let tx_stderr = action_tx;
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stderr.send(Action::AnvilLog(line));
            }
        });

        Ok(())
    }

    /// Start anvil in fork mode (no state load/dump)
    pub async fn start_fork(
        &mut self,
        port: u16,
        fork_url: &str,
        action_tx: UnboundedSender<Action>,
    ) -> color_eyre::Result<()> {
        self.cleanup_if_exited();
        if self.child.is_some() {
            return Err(eyre::eyre!("Anvil is already running"));
        }

        self.port = port;
        self.fork_url = Some(fork_url.to_string());

        let mut child = Command::new("anvil")
            .arg("--port").arg(port.to_string())
            .arg("--fork-url").arg(fork_url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");

        self.child = Some(child);

        // Reuse the same stdout parsing logic
        let tx_stdout = action_tx.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut accounts: Vec<String> = Vec::new();
            let mut parsing_accounts = false;
            let mut parsing_keys = false;
            let mut current_keys: Vec<String> = Vec::new();
            let mut started = false;

            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Available Accounts") {
                    parsing_accounts = true;
                    parsing_keys = false;
                    continue;
                }
                if line.contains("Private Keys") {
                    parsing_accounts = false;
                    parsing_keys = true;
                    continue;
                }
                if line.contains("Wallet") || line.contains("Base Fee") || line.contains("Listening") {
                    parsing_accounts = false;
                    parsing_keys = false;

                    if !accounts.is_empty() {
                        let mut full_accounts: Vec<AnvilAccount> = Vec::new();
                        for (i, addr) in accounts.iter().enumerate() {
                            full_accounts.push(AnvilAccount {
                                address: addr.clone(),
                                key: current_keys.get(i).cloned().unwrap_or_default(),
                                balance: "10000 ETH".to_string(),
                            });
                        }
                        let _ = tx_stdout.send(Action::AnvilAccounts(full_accounts));
                        accounts.clear();
                        current_keys.clear();
                    }
                }

                if line.contains("Listening on") {
                    started = true;
                    let _ = tx_stdout.send(Action::AnvilStarted { port });
                }

                if parsing_accounts {
                    if let Some(addr) = parse_anvil_account(&line) {
                        accounts.push(addr);
                    }
                }

                if parsing_keys {
                    if let Some(key) = parse_anvil_key(&line) {
                        current_keys.push(key);
                    }
                }

                let _ = tx_stdout.send(Action::AnvilLog(line));
            }

            // stdout EOF — process exited
            if !started {
                let _ = tx_stdout.send(Action::AnvilError("Anvil process exited unexpectedly".to_string()));
            }
            let _ = tx_stdout.send(Action::AnvilStopped);
        });

        let tx_stderr = action_tx;
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stderr.send(Action::AnvilLog(line));
            }
        });

        Ok(())
    }

    /// Stop the running anvil instance (dumps state before killing)
    pub async fn stop(&mut self) -> color_eyre::Result<()> {
        if self.child.is_some() && self.fork_url.is_none() {
            // Best-effort dump before stopping (skip for fork mode)
            let _ = Self::dump_state(self.port).await;
        }
        if let Some(mut child) = self.child.take() {
            child.kill().await?;
            child.wait().await?;
        }
        self.fork_url = None;
        Ok(())
    }

    /// Mine a single block by calling anvil RPC
    pub async fn mine_block(port: u16) -> color_eyre::Result<u64> {
        let output = Command::new("cast")
            .args(["rpc", "evm_mine", "--rpc-url"])
            .arg(format!("http://localhost:{}", port))
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to mine block: {}", err));
        }

        // Get current block number
        let output = Command::new("cast")
            .args(["block-number", "--rpc-url"])
            .arg(format!("http://localhost:{}", port))
            .output()
            .await?;

        let block_num = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .unwrap_or(0);

        Ok(block_num)
    }

    /// Reset anvil state
    pub async fn reset(port: u16) -> color_eyre::Result<()> {
        let output = Command::new("cast")
            .args(["rpc", "anvil_reset", "--rpc-url"])
            .arg(format!("http://localhost:{}", port))
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to reset: {}", err));
        }

        Ok(())
    }

    /// Dump anvil state via anvil_dumpState RPC and save to file
    pub async fn dump_state(port: u16) -> color_eyre::Result<()> {
        let rpc_url = format!("http://localhost:{}", port);
        let output = Command::new("cast")
            .args(["rpc", "anvil_dumpState", "--rpc-url", &rpc_url])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to dump state: {}", err));
        }

        let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let path = Self::state_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &state)?;
        Ok(())
    }

    /// Load anvil state from file via anvil_loadState RPC
    pub async fn load_state(port: u16) -> color_eyre::Result<()> {
        let path = Self::state_path();
        if !path.exists() {
            return Err(eyre::eyre!("No state file found at {:?}", path));
        }
        let state = std::fs::read_to_string(&path)?;
        let rpc_url = format!("http://localhost:{}", port);

        // anvil_loadState takes the hex-encoded state as a single param
        let output = Command::new("cast")
            .args(["rpc", "anvil_loadState", &state, "--rpc-url", &rpc_url])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to load state: {}", err));
        }

        Ok(())
    }
}

impl Drop for AnvilManager {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}

fn parse_anvil_account(line: &str) -> Option<String> {
    // Format: (0) 0x1234...5678 (10000.000000000000000000 ETH)
    let line = line.trim();
    if !line.starts_with('(') {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[1].starts_with("0x") {
        Some(parts[1].to_string())
    } else {
        None
    }
}

fn parse_anvil_key(line: &str) -> Option<String> {
    let line = line.trim();
    if !line.starts_with('(') {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 && parts[1].starts_with("0x") {
        Some(parts[1].to_string())
    } else {
        None
    }
}
