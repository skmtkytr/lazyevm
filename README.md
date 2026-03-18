# lazyevm

[![CI](https://github.com/skmtkytr/lazyevm/actions/workflows/ci.yml/badge.svg)](https://github.com/skmtkytr/lazyevm/actions/workflows/ci.yml)
[![Release](https://github.com/skmtkytr/lazyevm/actions/workflows/release.yml/badge.svg)](https://github.com/skmtkytr/lazyevm/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A **lazygit-style TUI** for Foundry EVM local development. Manage Anvil instances, wallets, forge builds, cast commands, and block exploration — all from a single terminal interface.

<!-- TODO: Add screenshot -->
<!-- ![screenshot](docs/screenshot.png) -->

## Features

- **Anvil Management** — Start/stop Anvil, fork mainnet, mine blocks, reset state, dump/load snapshots
- **Wallet Management** — List cast wallets, create/import keystores, unlock & check balances
- **Network Switching** — Configure and switch between RPC endpoints (local, mainnet, testnets)
- **Token Dashboard** — Track ERC-20 token balances, auto-detect storage slots, deal tokens on forks
- **Forge Integration** — Run `forge build` and `forge test` with colorized streaming output
- **Cast Commands** — Interactive `cast call`, `cast send`, and `cast balance` forms
- **Block Explorer** — Browse blocks, transactions, and transaction details on connected nodes
- **Catppuccin Theme** — Dark, easy-on-the-eyes color scheme

## Prerequisites

- [Rust](https://rustup.rs/) (1.70+)
- [Foundry](https://getfoundry.sh/) (`anvil`, `forge`, `cast` in PATH)

## Installation

### From GitHub Releases

Download a prebuilt binary from the [Releases](https://github.com/skmtkytr/lazyevm/releases) page.

```bash
# macOS (Apple Silicon)
curl -L https://github.com/skmtkytr/lazyevm/releases/latest/download/lazyevm-aarch64-darwin.tar.gz | tar xz
sudo mv lazyevm /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/skmtkytr/lazyevm/releases/latest/download/lazyevm-x86_64-darwin.tar.gz | tar xz
sudo mv lazyevm /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/skmtkytr/lazyevm/releases/latest/download/lazyevm-x86_64-linux.tar.gz | tar xz
sudo mv lazyevm /usr/local/bin/
```

### From source

```bash
cargo install --git https://github.com/skmtkytr/lazyevm
```

### Build locally

```bash
git clone https://github.com/skmtkytr/lazyevm.git
cd lazyevm
cargo build --release
./target/release/lazyevm
```

## Quick Start

```bash
lazyevm
```

1. Press `s` to start Anvil
2. Press `f` to start Anvil in fork mode (uses configured fork URL)
3. Navigate to **Wallets** panel to see accounts and balances
4. Navigate to **Forge** panel, press `b` to build or `t` to test
5. Use **Cast** panel for interactive contract calls
6. Browse blocks and transactions in **Explorer**

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Ctrl+C` | Force quit |
| `1`-`5` | Switch panel |
| `?` | Toggle help |
| `[` / `]` | Previous / Next sub-tab |

### Navigation

| Key | Action |
|-----|--------|
| `j` / `k` | Move down / up |
| `l` / `Enter` | Select / Focus content |
| `h` / `Esc` | Back / Focus sidebar |

### Anvil Panel

| Key | Action |
|-----|--------|
| `s` | Start Anvil |
| `S` | Stop Anvil |
| `f` | Start Anvil (fork mode) |
| `F` | Edit fork URL |
| `m` | Mine a block |
| `R` | Reset Anvil state |
| `t` | Transfer ETH/tokens between accounts |
| `D` | Deal ERC-20 tokens (fork mode, Tokens tab) |
| `E` | Deal ETH (fork mode) |
| `B` | Auto-detect token balance slot |
| `a` | Add token to watchlist |
| `x` | Remove token from watchlist |
| `r` | Refresh token balances |
| `d` | Dump Anvil state |
| `L` | Load Anvil state |

### Wallets Panel

| Key | Action |
|-----|--------|
| `n` | Create new wallet |
| `i` | Import wallet |
| `r` | Refresh wallets |
| `p` | Enter password (unlock locked wallet) |

### Wallets — Networks Tab

| Key | Action |
|-----|--------|
| `l` / `Enter` | Select active network |
| `n` | Add network |
| `d` | Delete network |

### Forge Panel

| Key | Action |
|-----|--------|
| `b` | Run `forge build` |
| `t` | Run `forge test` |
| `c` | Clear output |

### Cast Panel

| Key | Action |
|-----|--------|
| `Enter` | Start editing / Execute command |
| `j` / `k` | Switch input field |
| `Esc` | Stop editing |

### Explorer Panel

| Key | Action |
|-----|--------|
| `r` | Refresh blocks |
| `Enter` | View block transactions / Transaction detail |
| `Esc` | Back to previous view |

## Configuration

Config file: `~/.config/lazyevm/config.toml`

```toml
[networks]
active = "Anvil Local"

[[networks.list]]
name = "Anvil Local"
url = "http://localhost:8545"

[[networks.list]]
name = "Ethereum Mainnet"
url = "https://eth.llamarpc.com"

[[networks.list]]
name = "Sepolia"
url = "https://rpc.sepolia.org"

[anvil]
fork_network = "Ethereum Mainnet"
# fork_url = "https://your-rpc-url.com"

[[tokens.list]]
address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
symbol = "USDC"
name = "USD Coin"
decimals = 6
balance_slot = 9

[[tokens.list]]
address = "0xdAC17F958D2ee523a2206206994597C13D831ec7"
symbol = "USDT"
name = "Tether USD"
decimals = 6
balance_slot = 2
```

You can also set the `ETH_RPC_URL` environment variable — it will be added as the first network entry automatically.

## License

[MIT](LICENSE)
