use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;

pub struct ForgeRunner;

impl ForgeRunner {
    /// Run `forge build` with streaming output
    pub async fn build(action_tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        Self::run_command("build", &[], action_tx).await
    }

    /// Run `forge test` with streaming output
    pub async fn test(action_tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        Self::run_command("test", &["-vv"], action_tx).await
    }

    /// Run `forge script` with streaming output
    pub async fn script(path: &str, action_tx: UnboundedSender<Action>) -> color_eyre::Result<()> {
        Self::run_command("script", &[path, "-vvvv"], action_tx).await
    }

    async fn run_command(
        subcmd: &str,
        args: &[&str],
        action_tx: UnboundedSender<Action>,
    ) -> color_eyre::Result<()> {
        let mut child = Command::new("forge")
            .arg(subcmd)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");

        let tx1 = action_tx.clone();
        let stdout_handle = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = strip_ansi(&line);
                let _ = tx1.send(Action::ForgeOutput(clean));
            }
        });

        let tx2 = action_tx.clone();
        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let clean = strip_ansi(&line);
                let _ = tx2.send(Action::ForgeOutput(clean));
            }
        });

        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        let status = child.wait().await?;
        let success = status.success();

        let summary = if success {
            format!("forge {} completed successfully", subcmd)
        } else {
            format!("forge {} failed with exit code {:?}", subcmd, status.code())
        };

        let _ = action_tx.send(Action::ForgeDone { success, summary });

        Ok(())
    }
}

fn strip_ansi(s: &str) -> String {
    strip_ansi_escapes::strip_str(s).to_string()
}
