use std::time::Duration;
use tokio::process::Command;

/// Timeout for cast CLI commands (30 seconds)
const CAST_TIMEOUT: Duration = Duration::from_secs(30);

/// Well-known ERC20 balance storage slots (address -> slot)
const KNOWN_SLOTS: &[(&str, u64)] = &[
    // Major stablecoins
    ("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 9),  // USDC
    ("0xdAC17F958D2ee523a2206206994597C13D831ec7", 2),  // USDT
    ("0x6B175474E89094C44Da98b954EedeAC495271d0F", 2),  // DAI
    ("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 3),  // WETH
    // DeFi tokens
    ("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984", 4),  // UNI
    ("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9", 0),  // AAVE
    ("0xc00e94Cb662C3520282E6f5717214004A7f26888", 1),  // COMP
    ("0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2", 1),  // MKR
    ("0x514910771AF9Ca656af840dff83E8264EcF986CA", 1),  // LINK
    ("0x6B3595068778DD592e39A122f4f5a5cF09C90fE2", 0),  // SUSHI
    // Wrapped / bridged
    ("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", 0),  // WBTC
    ("0x4Fabb145d64652a948d72533023f6E7A623C7C53", 1),  // BUSD
    ("0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE", 0),  // SHIB
    ("0x853d955aCEf822Db058eb8505911ED77F175b99e", 0),  // FRAX
];

pub struct CastRunner {
    pub rpc_url: String,
}

impl CastRunner {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
        }
    }

    /// Run a cast command with timeout
    async fn run_cast(cmd: &mut Command) -> color_eyre::Result<String> {
        let output = tokio::time::timeout(CAST_TIMEOUT, cmd.output())
            .await
            .map_err(|_| eyre::eyre!("cast command timed out after {}s", CAST_TIMEOUT.as_secs()))?
            ?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("{}", err.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Run `cast call`
    pub async fn call(
        &self,
        to: &str,
        sig: &str,
        args: &[String],
    ) -> color_eyre::Result<String> {
        let mut cmd = Command::new("cast");
        cmd.args(["call", to, sig]);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.args(["--rpc-url", &self.rpc_url]);
        Self::run_cast(&mut cmd).await
    }

    /// Run `cast send`
    pub async fn send(
        &self,
        to: &str,
        sig: &str,
        args: &[String],
    ) -> color_eyre::Result<String> {
        let mut cmd = Command::new("cast");
        cmd.args(["send", to, sig]);
        for arg in args {
            cmd.arg(arg);
        }
        cmd.args(["--rpc-url", &self.rpc_url]);
        // For anvil, use the first default account
        cmd.args([
            "--private-key",
            "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
        ]);
        Self::run_cast(&mut cmd).await
    }

    /// Run `cast send` for ETH transfer (no function sig)
    pub async fn transfer(
        &self,
        from_key: &str,
        to: &str,
        value_ether: &str,
    ) -> color_eyre::Result<String> {
        let mut cmd = Command::new("cast");
        cmd.args([
            "send",
            to,
            "--value",
            &format!("{}ether", value_ether),
            "--private-key",
            from_key,
            "--rpc-url",
            &self.rpc_url,
        ]);
        Self::run_cast(&mut cmd).await
    }

    /// Run `cast send` for ERC20 transfer
    pub async fn transfer_erc20(
        &self,
        from_key: &str,
        token: &str,
        to: &str,
        amount: &str,
    ) -> color_eyre::Result<String> {
        let mut cmd = Command::new("cast");
        cmd.args([
            "send",
            token,
            "transfer(address,uint256)",
            to,
            amount,
            "--private-key",
            from_key,
            "--rpc-url",
            &self.rpc_url,
        ]);
        Self::run_cast(&mut cmd).await
    }

    /// Query ERC20 token balance
    pub async fn token_balance(&self, token: &str, account: &str) -> color_eyre::Result<String> {
        self.call(token, "balanceOf(address)(uint256)", &[account.to_string()]).await
    }

    /// Query ERC20 token symbol
    pub async fn token_symbol(&self, token: &str) -> color_eyre::Result<String> {
        self.call(token, "symbol()(string)", &[]).await
    }

    /// Query ERC20 token name
    pub async fn token_name(&self, token: &str) -> color_eyre::Result<String> {
        self.call(token, "name()(string)", &[]).await
    }

    /// Query ERC20 token decimals
    pub async fn token_decimals(&self, token: &str) -> color_eyre::Result<u8> {
        let result = self.call(token, "decimals()(uint8)", &[]).await?;
        result.trim().parse::<u8>().map_err(|e| eyre::eyre!("parse decimals: {}", e))
    }

    /// Deal token: set balance via anvil_setStorageAt, then verify
    pub async fn deal_token(
        &self,
        token: &str,
        account: &str,
        amount_raw: &str,
        balance_slot: u64,
    ) -> color_eyre::Result<String> {
        // Validate inputs
        if !account.starts_with("0x") || account.len() != 42 {
            return Err(eyre::eyre!("Invalid account address: {}", account));
        }
        if !token.starts_with("0x") || token.len() != 42 {
            return Err(eyre::eyre!("Invalid token address: {}", token));
        }

        // 1. Compute storage key: keccak256(abi.encode(account, slot))
        let mut cmd = Command::new("cast");
        cmd.args(["index", "address", account, &balance_slot.to_string()]);
        let storage_key = Self::run_cast(&mut cmd).await
            .map_err(|e| eyre::eyre!("cast index failed: {}", e))?;

        // 2. Convert amount to uint256 hex
        let mut cmd = Command::new("cast");
        cmd.args(["to-uint256", amount_raw]);
        let hex_value = Self::run_cast(&mut cmd).await
            .map_err(|e| eyre::eyre!("cast to-uint256 failed: {}", e))?;

        // 3. Set storage via anvil_setStorageAt
        let mut cmd = Command::new("cast");
        cmd.args([
            "rpc",
            "anvil_setStorageAt",
            token,
            &storage_key,
            &hex_value,
            "--rpc-url",
            &self.rpc_url,
        ]);
        Self::run_cast(&mut cmd).await
            .map_err(|e| eyre::eyre!("anvil_setStorageAt failed: {}", e))?;

        // 4. Verify: read back balance
        match self.token_balance(token, account).await {
            Ok(new_bal) => {
                if new_bal.trim() == "0" && amount_raw != "0" {
                    return Err(eyre::eyre!(
                        "Deal may have failed: balance is 0 after setting {}. \
                         The balance_slot ({}) might be incorrect.",
                        amount_raw, balance_slot
                    ));
                }
                Ok(format!("Deal set (verified balance: {})", new_bal.trim()))
            }
            Err(_) => {
                // Verification failed but the set may have worked
                Ok(format!("Deal set for {} (verification skipped)", token))
            }
        }
    }

    /// Set ETH balance directly via anvil_setBalance
    pub async fn set_eth_balance(
        &self,
        account: &str,
        amount_wei: &str,
    ) -> color_eyre::Result<String> {
        if !account.starts_with("0x") || account.len() != 42 {
            return Err(eyre::eyre!("Invalid account address: {}", account));
        }

        // Convert to hex
        let mut cmd = Command::new("cast");
        cmd.args(["to-uint256", amount_wei]);
        let hex_value = Self::run_cast(&mut cmd).await
            .map_err(|e| eyre::eyre!("cast to-uint256 failed: {}", e))?;

        let mut cmd = Command::new("cast");
        cmd.args([
            "rpc",
            "anvil_setBalance",
            account,
            &hex_value,
            "--rpc-url",
            &self.rpc_url,
        ]);
        Self::run_cast(&mut cmd).await
            .map_err(|e| eyre::eyre!("anvil_setBalance failed: {}", e))?;

        Ok(format!("ETH balance set for {}", account))
    }

    /// Detect balance storage slot by brute-forcing slots 0-20.
    /// Sets a known value, reads balanceOf, checks if it matches.
    pub async fn detect_balance_slot(
        &self,
        token: &str,
        test_account: &str,
    ) -> color_eyre::Result<u64> {
        // First check well-known slots
        if let Some(slot) = lookup_known_slot(token) {
            return Ok(slot);
        }

        // Save original balance to restore
        let original = self.token_balance(token, test_account).await
            .unwrap_or_else(|_| "0".to_string());

        let test_value = "1337420";

        for slot in 0..=20 {
            // Compute storage key
            let mut cmd = Command::new("cast");
            cmd.args(["index", "address", test_account, &slot.to_string()]);
            let storage_key = match Self::run_cast(&mut cmd).await {
                Ok(k) => k,
                Err(_) => continue,
            };

            // Convert test value to uint256
            let mut cmd = Command::new("cast");
            cmd.args(["to-uint256", test_value]);
            let hex_value = match Self::run_cast(&mut cmd).await {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Set storage
            let mut cmd = Command::new("cast");
            cmd.args([
                "rpc", "anvil_setStorageAt", token,
                &storage_key, &hex_value,
                "--rpc-url", &self.rpc_url,
            ]);
            if Self::run_cast(&mut cmd).await.is_err() {
                continue;
            }

            // Read balance
            if let Ok(bal) = self.token_balance(token, test_account).await {
                if bal.trim() == test_value {
                    // Found it! Restore original balance
                    let mut cmd = Command::new("cast");
                    cmd.args(["to-uint256", original.trim()]);
                    if let Ok(orig_hex) = Self::run_cast(&mut cmd).await {
                        let mut cmd = Command::new("cast");
                        cmd.args([
                            "rpc", "anvil_setStorageAt", token,
                            &storage_key, &orig_hex,
                            "--rpc-url", &self.rpc_url,
                        ]);
                        let _ = Self::run_cast(&mut cmd).await;
                    }
                    return Ok(slot);
                }
            }
        }

        Err(eyre::eyre!("Could not detect balance slot (tried 0-20)"))
    }

    /// Check if fork RPC URL is reachable
    pub async fn check_rpc_connectivity(url: &str) -> color_eyre::Result<()> {
        let mut cmd = Command::new("cast");
        cmd.args(["chain-id", "--rpc-url", url]);
        let result = tokio::time::timeout(Duration::from_secs(10), cmd.output())
            .await
            .map_err(|_| eyre::eyre!("RPC connection timed out: {}", url))?
            ?;

        if !result.status.success() {
            let err = String::from_utf8_lossy(&result.stderr);
            return Err(eyre::eyre!("RPC unreachable: {}", err.trim()));
        }
        Ok(())
    }

    /// Format raw token balance to human-readable with decimals
    pub fn format_token_balance(raw: &str, decimals: u8) -> String {
        let raw = raw.trim();
        if raw.is_empty() || raw == "0" {
            return "0".to_string();
        }

        let dec = decimals as usize;
        if dec == 0 {
            return raw.to_string();
        }

        let raw_str = raw.to_string();

        if raw_str.len() <= dec {
            let padding = dec - raw_str.len();
            let fractional = format!("{}{}", "0".repeat(padding), raw_str);
            let trimmed = fractional.trim_end_matches('0');
            if trimmed.is_empty() {
                "0".to_string()
            } else {
                format!("0.{}", trimmed)
            }
        } else {
            let split = raw_str.len() - dec;
            let integer = &raw_str[..split];
            let fractional = &raw_str[split..];
            let trimmed = fractional.trim_end_matches('0');
            if trimmed.is_empty() {
                integer.to_string()
            } else {
                format!("{}.{}", integer, trimmed)
            }
        }
    }

    /// Convert human-readable amount to raw amount (e.g., "1.5" with 6 decimals -> "1500000")
    pub fn compute_raw_amount(amount: &str, decimals: u8) -> String {
        let amount = amount.trim();
        let dec = decimals as usize;

        if let Some(dot_pos) = amount.find('.') {
            let integer = &amount[..dot_pos];
            let fractional = &amount[dot_pos + 1..];

            if fractional.len() >= dec {
                format!("{}{}", integer, &fractional[..dec])
            } else {
                let padding = dec - fractional.len();
                format!("{}{}{}", integer, fractional, "0".repeat(padding))
            }
        } else {
            format!("{}{}", amount, "0".repeat(dec))
        }
    }

    /// Validate Ethereum address format
    pub fn is_valid_address(addr: &str) -> bool {
        addr.starts_with("0x")
            && addr.len() == 42
            && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Run `cast balance`
    pub async fn balance(&self, address: &str) -> color_eyre::Result<String> {
        let mut cmd = Command::new("cast");
        cmd.args([
            "balance",
            address,
            "--ether",
            "--rpc-url",
            &self.rpc_url,
        ]);
        let balance = Self::run_cast(&mut cmd).await?;
        Ok(format!("{} ETH", balance))
    }
}

/// Look up a well-known balance slot by token address (case-insensitive)
pub fn lookup_known_slot(token: &str) -> Option<u64> {
    let token_lower = token.to_lowercase();
    KNOWN_SLOTS.iter()
        .find(|(addr, _)| addr.to_lowercase() == token_lower)
        .map(|(_, slot)| *slot)
}
