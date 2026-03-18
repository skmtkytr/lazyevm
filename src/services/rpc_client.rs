use tokio::process::Command;

use crate::action::{BlockInfo, TxDetail, TxInfo};

pub struct RpcClient {
    pub rpc_url: String,
}

impl RpcClient {
    pub fn new(rpc_url: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
        }
    }

    pub fn with_port(port: u16) -> Self {
        Self {
            rpc_url: format!("http://localhost:{}", port),
        }
    }

    /// Get balance of an address using cast
    pub async fn get_balance(&self, address: &str) -> color_eyre::Result<String> {
        let output = Command::new("cast")
            .args(["balance", address, "--ether", "--rpc-url", &self.rpc_url])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to get balance: {}", err));
        }

        let balance = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(format!("{} ETH", balance))
    }

    /// Get latest block number
    pub async fn get_block_number(&self) -> color_eyre::Result<u64> {
        let output = Command::new("cast")
            .args(["block-number", "--rpc-url", &self.rpc_url])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(0);
        }

        let num = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u64>()
            .unwrap_or(0);

        Ok(num)
    }

    /// Get recent blocks
    pub async fn get_recent_blocks(&self, count: u64) -> color_eyre::Result<Vec<BlockInfo>> {
        let latest = self.get_block_number().await?;
        let mut blocks = Vec::new();

        let start = latest.saturating_sub(count.saturating_sub(1));
        for num in (start..=latest).rev() {
            if let Ok(block) = self.get_block(num).await {
                blocks.push(block);
            }
        }

        Ok(blocks)
    }

    /// Get block info
    pub async fn get_block(&self, number: u64) -> color_eyre::Result<BlockInfo> {
        let output = Command::new("cast")
            .args([
                "block",
                &number.to_string(),
                "--json",
                "--rpc-url",
                &self.rpc_url,
            ])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to get block: {}", err));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&json_str)?;

        let hash = json["hash"].as_str().unwrap_or("").to_string();
        let timestamp = u64::from_str_radix(
            json["timestamp"]
                .as_str()
                .unwrap_or("0x0")
                .trim_start_matches("0x"),
            16,
        )
        .unwrap_or(0);
        let tx_count = json["transactions"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0);
        let gas_used = json["gasUsed"].as_str().unwrap_or("0").to_string();

        Ok(BlockInfo {
            number,
            hash,
            timestamp,
            tx_count,
            gas_used,
        })
    }

    /// Get transactions in a block
    pub async fn get_block_transactions(&self, number: u64) -> color_eyre::Result<Vec<TxInfo>> {
        let output = Command::new("cast")
            .args([
                "block",
                &number.to_string(),
                "--json",
                "--rpc-url",
                &self.rpc_url,
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&json_str)?;

        let mut txs = Vec::new();
        if let Some(transactions) = json["transactions"].as_array() {
            for tx_hash in transactions {
                if let Some(hash) = tx_hash.as_str() {
                    if let Ok(tx) = self.get_tx_info(hash).await {
                        txs.push(tx);
                    }
                }
            }
        }

        Ok(txs)
    }

    /// Get transaction info
    async fn get_tx_info(&self, hash: &str) -> color_eyre::Result<TxInfo> {
        let output = Command::new("cast")
            .args(["tx", hash, "--json", "--rpc-url", &self.rpc_url])
            .output()
            .await?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(eyre::eyre!("Failed to get tx: {}", err));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&json_str)?;

        Ok(TxInfo {
            hash: hash.to_string(),
            from: json["from"].as_str().unwrap_or("").to_string(),
            to: json["to"].as_str().unwrap_or("").to_string(),
            value: json["value"].as_str().unwrap_or("0").to_string(),
            status: true,
        })
    }

    /// Get full transaction details
    pub async fn get_tx_detail(&self, hash: &str) -> color_eyre::Result<TxDetail> {
        let tx_output = Command::new("cast")
            .args(["tx", hash, "--json", "--rpc-url", &self.rpc_url])
            .output()
            .await?;

        let receipt_output = Command::new("cast")
            .args(["receipt", hash, "--json", "--rpc-url", &self.rpc_url])
            .output()
            .await?;

        let tx_json: serde_json::Value = if tx_output.status.success() {
            serde_json::from_str(&String::from_utf8_lossy(&tx_output.stdout)).unwrap_or_default()
        } else {
            serde_json::Value::Null
        };

        let receipt_json: serde_json::Value = if receipt_output.status.success() {
            serde_json::from_str(&String::from_utf8_lossy(&receipt_output.stdout))
                .unwrap_or_default()
        } else {
            serde_json::Value::Null
        };

        let status = receipt_json["status"]
            .as_str()
            .map(|s| s == "0x1" || s == "1")
            .unwrap_or(true);

        let block_number = u64::from_str_radix(
            tx_json["blockNumber"]
                .as_str()
                .unwrap_or("0x0")
                .trim_start_matches("0x"),
            16,
        )
        .unwrap_or(0);

        Ok(TxDetail {
            hash: hash.to_string(),
            from: tx_json["from"].as_str().unwrap_or("").to_string(),
            to: tx_json["to"].as_str().unwrap_or("").to_string(),
            value: tx_json["value"].as_str().unwrap_or("0").to_string(),
            gas_used: receipt_json["gasUsed"]
                .as_str()
                .unwrap_or("0")
                .to_string(),
            gas_price: tx_json["gasPrice"]
                .as_str()
                .unwrap_or("0")
                .to_string(),
            input: tx_json["input"].as_str().unwrap_or("0x").to_string(),
            block_number,
            status,
        })
    }
}
