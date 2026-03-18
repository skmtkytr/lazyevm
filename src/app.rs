use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::action::Action;
use crate::components::status_bar::{MessageKind, StatusBar};
use crate::components::sidebar::Sidebar;
use crate::components::Component;
use crate::config::Config;
use crate::event::{Event, EventHandler};
use crate::panels::PanelId;
use crate::panels::anvil::AnvilPanel;
use crate::panels::cast::CastPanel;
use crate::panels::explorer::ExplorerPanel;
use crate::panels::forge::ForgePanel;
use crate::panels::wallets::WalletsPanel;
use crate::services::anvil_manager::AnvilManager;
use crate::services::cast_runner::CastRunner;
use crate::services::forge_runner::ForgeRunner;
use crate::services::keystore::KeystoreService;
use crate::services::rpc_client::RpcClient;
use crate::tui;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    Sidebar,
    Content,
}

pub struct App {
    running: bool,
    active_panel: PanelId,
    focus: Focus,
    show_help: bool,

    // UI components
    sidebar: Sidebar,
    status_bar: StatusBar,

    // Panels
    wallets: WalletsPanel,
    anvil: AnvilPanel,
    forge: ForgePanel,
    cast: CastPanel,
    explorer: ExplorerPanel,

    // Services
    anvil_manager: AnvilManager,

    // Config
    config: Config,

    // Action channel
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
}

impl App {
    pub fn new() -> Self {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let config = Config::load();

        Self {
            running: true,
            active_panel: PanelId::Wallets,
            focus: Focus::Content,
            show_help: false,

            sidebar: Sidebar::new(),
            status_bar: StatusBar::new(),

            wallets: WalletsPanel::new(),
            anvil: AnvilPanel::new(),
            forge: ForgePanel::new(),
            cast: CastPanel::new(),
            explorer: ExplorerPanel::new(),

            anvil_manager: AnvilManager::new(),

            config,

            action_tx,
            action_rx,
        }
    }

    pub async fn run(&mut self) -> color_eyre::Result<()> {
        let mut terminal = tui::init()?;
        let mut events = EventHandler::new(250, 50);

        // Initialize panels with action sender
        self.wallets.init(self.action_tx.clone());
        self.anvil.init(self.action_tx.clone());
        self.forge.init(self.action_tx.clone());
        self.cast.init(self.action_tx.clone());
        self.explorer.init(self.action_tx.clone());

        // Pass config data to panels
        self.wallets.set_networks(
            self.config.networks.list.clone(),
            self.config.networks.active.clone(),
        );
        self.status_bar.network_name = self.config.active_network_name().to_string();

        // Initialize token and fork config
        self.anvil.set_tokens(&self.config.tokens.list);
        if let Some(url) = self.config.fork_rpc_url() {
            self.anvil.set_fork_url(url.to_string());
        }

        while self.running {
            // Handle events
            tokio::select! {
                event = events.next() => {
                    match event? {
                        Event::Key(key) => {
                            self.handle_key(key);
                        }
                        Event::Tick => {
                            let _ = self.action_tx.send(Action::Tick);
                        }
                        Event::Render => {
                            terminal.draw(|frame| self.draw(frame))?;
                        }
                        Event::Resize(_, _) => {
                            terminal.draw(|frame| self.draw(frame))?;
                        }
                    }
                }
                Some(action) = self.action_rx.recv() => {
                    self.dispatch(action).await;
                }
            }
        }

        tui::restore()?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always force-quits
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let _ = self.action_tx.send(Action::Quit);
            return;
        }

        // Help overlay intercept
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {}
            }
            return;
        }

        // Toggle help
        if key.code == KeyCode::Char('?') {
            self.show_help = true;
            return;
        }

        if self.focus == Focus::Sidebar {
            self.handle_sidebar_key(key);
        } else {
            self.handle_content_key(key);
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
        let action = match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            // hjkl navigation
            KeyCode::Char('j') | KeyCode::Down => {
                let next = (self.sidebar.active + 1).min(self.sidebar.panels.len() - 1);
                let panel = self.sidebar.panels[next];
                Some(Action::SwitchPanel(panel))
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let prev = self.sidebar.active.saturating_sub(1);
                let panel = self.sidebar.panels[prev];
                Some(Action::SwitchPanel(panel))
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                Some(Action::FocusContent)
            }
            // Panel switching by number
            KeyCode::Char('1') => Some(Action::SwitchPanel(PanelId::Wallets)),
            KeyCode::Char('2') => Some(Action::SwitchPanel(PanelId::Anvil)),
            KeyCode::Char('3') => Some(Action::SwitchPanel(PanelId::Forge)),
            KeyCode::Char('4') => Some(Action::SwitchPanel(PanelId::Cast)),
            KeyCode::Char('5') => Some(Action::SwitchPanel(PanelId::Explorer)),
            _ => None,
        };
        if let Some(action) = action {
            let _ = self.action_tx.send(action);
        }
    }

    fn handle_content_key(&mut self, key: KeyEvent) {
        // Panel gets first chance to handle the key.
        // Returns Some(action) = consumed (dispatch action),
        //         None = not consumed (fall through to default nav).
        let panel_action = match self.active_panel {
            PanelId::Wallets => self.wallets.handle_key_events(key),
            PanelId::Anvil => self.anvil.handle_key_events(key),
            PanelId::Forge => self.forge.handle_key_events(key),
            PanelId::Cast => self.cast.handle_key_events(key),
            PanelId::Explorer => self.explorer.handle_key_events(key),
        };
        if let Some(action) = panel_action {
            let _ = self.action_tx.send(action);
            return;
        }

        // Default navigation — only reached when panel didn't consume the key
        let action = match key.code {
            KeyCode::Char('q') => Some(Action::Quit),
            // hjkl lazygit-style
            KeyCode::Char('h') => Some(Action::PrevTab),
            KeyCode::Char('l') => Some(Action::NextTab),
            KeyCode::Char('j') | KeyCode::Down => Some(Action::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(Action::Up),
            KeyCode::Enter => Some(Action::Select),
            KeyCode::Esc | KeyCode::Left => Some(Action::FocusSidebar),
            KeyCode::Right => Some(Action::NextTab),
            // Panel switching by number
            KeyCode::Char('1') => Some(Action::SwitchPanel(PanelId::Wallets)),
            KeyCode::Char('2') => Some(Action::SwitchPanel(PanelId::Anvil)),
            KeyCode::Char('3') => Some(Action::SwitchPanel(PanelId::Forge)),
            KeyCode::Char('4') => Some(Action::SwitchPanel(PanelId::Cast)),
            KeyCode::Char('5') => Some(Action::SwitchPanel(PanelId::Explorer)),
            _ => None,
        };
        if let Some(action) = action {
            let _ = self.action_tx.send(action);
        }
    }

    async fn dispatch(&mut self, action: Action) {
        match action {
            Action::Quit => {
                // Stop anvil if running before quitting
                if self.anvil_manager.is_running() {
                    let _ = self.anvil_manager.stop().await;
                }
                self.running = false;
                return;
            }
            Action::SwitchPanel(panel) => {
                self.active_panel = panel;
                self.sidebar.select(panel);
                self.status_bar.active_panel = panel;
                return;
            }
            Action::FocusSidebar => {
                self.focus = Focus::Sidebar;
                self.sidebar.focused = true;
                return;
            }
            Action::FocusContent => {
                self.focus = Focus::Content;
                self.sidebar.focused = false;
                return;
            }
            Action::SetStatus(ref msg) => {
                self.status_bar.set_message(msg.clone(), MessageKind::Info);
            }
            Action::Error(ref msg) => {
                self.status_bar.set_message(msg.clone(), MessageKind::Error);
            }
            Action::ClearStatus => {
                self.status_bar.clear_message();
            }
            Action::Tick | Action::Render | Action::None => return,

            // ---- Async service dispatches ----

            // Wallet actions
            Action::RefreshWallets => {
                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    match KeystoreService::list_wallets().await {
                        Ok(wallets) => { let _ = tx.send(Action::WalletsLoaded(wallets)); }
                        Err(e) => { let _ = tx.send(Action::Error(e.to_string())); }
                    }
                });
            }
            Action::CreateWallet => {
                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    match KeystoreService::create_wallet().await {
                        Ok(msg) => {
                            let _ = tx.send(Action::WalletCreated(msg));
                            let _ = tx.send(Action::RefreshWallets);
                        }
                        Err(e) => { let _ = tx.send(Action::Error(e.to_string())); }
                    }
                });
            }
            Action::ImportWallet => {
                // TODO: would need a text input popup for name and private key
                self.status_bar.set_message(
                    "Import wallet: use `cast wallet import` from CLI".to_string(),
                    MessageKind::Info,
                );
            }
            Action::WalletsLoaded(ref wallets) => {
                // Auto-fetch balance for each wallet that has an address
                let rpc_url = self.config.active_rpc_url().to_string();
                for w in wallets.iter() {
                    if !w.address.is_empty() {
                        let tx = self.action_tx.clone();
                        let addr = w.address.clone();
                        let rpc = rpc_url.clone();
                        tokio::spawn(async move {
                            match KeystoreService::get_balance(&addr, &rpc).await {
                                Ok(bal) => {
                                    let _ = tx.send(Action::BalanceLoaded {
                                        address: addr,
                                        balance: bal,
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(Action::BalanceLoaded {
                                        address: addr,
                                        balance: format!("err: {}", e),
                                    });
                                }
                            }
                        });
                    }
                }
                // Delegate to panel
            }
            Action::BalanceLoaded { .. } | Action::WalletCreated(_) | Action::WalletImported(_) => {
                // Delegate to panel
            }
            Action::UnlockWallet { ref name, ref password } => {
                let tx = self.action_tx.clone();
                let name = name.clone();
                let password = password.clone();
                let rpc_url = self.config.active_rpc_url().to_string();
                tokio::spawn(async move {
                    match KeystoreService::unlock_wallet(&name, &password).await {
                        Ok(address) => {
                            let _ = tx.send(Action::WalletAddressResolved {
                                name: name.clone(),
                                address: address.clone(),
                            });
                            let _ = tx.send(Action::SetStatus(
                                format!("Unlocked wallet: {}", name),
                            ));
                            // Fetch balance for the newly unlocked wallet
                            match KeystoreService::get_balance(&address, &rpc_url).await {
                                Ok(bal) => {
                                    let _ = tx.send(Action::BalanceLoaded {
                                        address,
                                        balance: bal,
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(Action::BalanceLoaded {
                                        address,
                                        balance: format!("err: {}", e),
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Action::Error(
                                format!("Unlock failed: {}", e),
                            ));
                        }
                    }
                });
                return;
            }
            Action::WalletAddressResolved { .. } => {
                // Delegate to panel
            }

            // Network actions
            Action::SelectNetwork(ref name) => {
                self.config.select_network(name);
                let name = self.config.active_network_name().to_string();
                let url = self.config.active_rpc_url().to_string();
                self.status_bar.network_name = name.clone();
                self.wallets.set_networks(
                    self.config.networks.list.clone(),
                    self.config.networks.active.clone(),
                );
                let _ = self.action_tx.send(Action::NetworkChanged { name, url });
                return;
            }
            Action::AddNetwork { ref name, ref url } => {
                self.config.add_network(name.clone(), url.clone());
                self.wallets.set_networks(
                    self.config.networks.list.clone(),
                    self.config.networks.active.clone(),
                );
                let _ = self.action_tx.send(Action::SetStatus(
                    format!("Added network: {}", name),
                ));
                return;
            }
            Action::RemoveNetwork(ref name) => {
                self.config.remove_network(name);
                self.status_bar.network_name = self.config.active_network_name().to_string();
                self.wallets.set_networks(
                    self.config.networks.list.clone(),
                    self.config.networks.active.clone(),
                );
                let _ = self.action_tx.send(Action::SetStatus(
                    "Network removed".to_string(),
                ));
                return;
            }
            Action::NetworkChanged { .. } => {
                // Delegate to panel
            }

            // Anvil actions
            Action::StartAnvil => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                match self.anvil_manager.start(port, tx.clone()).await {
                    Ok(()) => {
                        self.status_bar.set_message(
                            format!("Starting anvil on port {}...", port),
                            MessageKind::Info,
                        );
                    }
                    Err(e) => {
                        let _ = self.action_tx.send(Action::AnvilError(e.to_string()));
                    }
                }
                return;
            }
            Action::StopAnvil => {
                match self.anvil_manager.stop().await {
                    Ok(()) => {
                        let _ = self.action_tx.send(Action::AnvilStopped);
                    }
                    Err(e) => {
                        let _ = self.action_tx.send(Action::AnvilError(e.to_string()));
                    }
                }
                return;
            }
            Action::AnvilStarted { .. } => {
                self.status_bar.anvil_running = true;
            }
            Action::AnvilStopped => {
                self.status_bar.anvil_running = false;
                // Clean up child handle if the process exited on its own
                self.anvil_manager.cleanup_if_exited();
            }
            Action::AnvilError(ref msg) => {
                self.status_bar.set_message(msg.clone(), MessageKind::Error);
            }
            Action::MineBlock => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                tokio::spawn(async move {
                    match AnvilManager::mine_block(port).await {
                        Ok(num) => { let _ = tx.send(Action::BlockMined(num)); }
                        Err(e) => { let _ = tx.send(Action::AnvilError(e.to_string())); }
                    }
                });
                return;
            }
            Action::ResetAnvil => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                tokio::spawn(async move {
                    match AnvilManager::reset(port).await {
                        Ok(()) => {
                            let _ = tx.send(Action::SetStatus("Anvil state reset".to_string()));
                        }
                        Err(e) => { let _ = tx.send(Action::AnvilError(e.to_string())); }
                    }
                });
                return;
            }
            Action::AnvilDumpState => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                self.status_bar.set_message("Dumping state...".to_string(), MessageKind::Info);
                tokio::spawn(async move {
                    match AnvilManager::dump_state(port).await {
                        Ok(()) => {
                            let _ = tx.send(Action::SetStatus("State dumped to file".to_string()));
                        }
                        Err(e) => { let _ = tx.send(Action::AnvilError(e.to_string())); }
                    }
                });
                return;
            }
            Action::AnvilLoadState => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let accounts: Vec<_> = self.anvil.accounts().to_vec();
                self.status_bar.set_message("Loading state...".to_string(), MessageKind::Info);
                tokio::spawn(async move {
                    match AnvilManager::load_state(port).await {
                        Ok(()) => {
                            let _ = tx.send(Action::SetStatus("State loaded from file".to_string()));
                            // Refresh account balances
                            let rpc = format!("http://localhost:{}", port);
                            let runner = CastRunner::new(&rpc);
                            let mut updated = accounts;
                            for acc in updated.iter_mut() {
                                if let Ok(bal) = runner.balance(&acc.address).await {
                                    acc.balance = bal;
                                }
                            }
                            let _ = tx.send(Action::AnvilAccounts(updated));
                        }
                        Err(e) => { let _ = tx.send(Action::AnvilError(e.to_string())); }
                    }
                });
                return;
            }
            Action::AnvilTransfer { ref from_key, ref to, ref value, ref token } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let from_key = from_key.clone();
                let to = to.clone();
                let value = value.clone();
                let token = token.clone();
                self.status_bar.set_message("Sending transfer...".to_string(), MessageKind::Info);
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    match token {
                        Some(ref token_addr) => {
                            match runner.transfer_erc20(&from_key, token_addr, &to, &value).await {
                                Ok(_) => {
                                    let msg = format!("ERC20 transfer {} to {}", value, to);
                                    let _ = tx.send(Action::AnvilTransferDone(msg));
                                }
                                Err(e) => {
                                    let _ = tx.send(Action::AnvilError(e.to_string()));
                                }
                            }
                        }
                        None => {
                            match runner.transfer(&from_key, &to, &value).await {
                                Ok(_) => {
                                    let msg = format!("Transferred {} ETH to {}", value, to);
                                    let _ = tx.send(Action::AnvilTransferDone(msg));
                                }
                                Err(e) => {
                                    let _ = tx.send(Action::AnvilError(e.to_string()));
                                }
                            }
                        }
                    }
                });
                return;
            }
            Action::AnvilTransferDone(_) => {
                // Delegate to panel for log/status, then refresh balances
                self.anvil.update(&action);
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let accounts: Vec<_> = self.anvil.accounts().to_vec();
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    let mut updated = accounts;
                    for acc in updated.iter_mut() {
                        if let Ok(bal) = runner.balance(&acc.address).await {
                            acc.balance = bal;
                        }
                    }
                    let _ = tx.send(Action::AnvilAccounts(updated));
                });
                return;
            }

            // Fork actions
            Action::SetForkUrl(ref url) => {
                let url = url.clone();
                self.config.set_fork_url(url.clone());
                self.anvil.set_fork_url(if url.is_empty() {
                    // Fall back to network-resolved URL
                    self.config.fork_rpc_url().unwrap_or("").to_string()
                } else {
                    url.clone()
                });
                if url.is_empty() {
                    self.status_bar.set_message("Fork URL cleared".to_string(), MessageKind::Info);
                } else {
                    self.status_bar.set_message(format!("Fork URL set: {}", url), MessageKind::Success);
                }
                return;
            }
            Action::StartAnvilFork { ref fork_url } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let fork_url = fork_url.clone();
                self.status_bar.set_message(
                    format!("Checking RPC connectivity {}...", fork_url),
                    MessageKind::Info,
                );
                // Validate URL first, then start fork
                let fork_url2 = fork_url.clone();
                match CastRunner::check_rpc_connectivity(&fork_url).await {
                    Ok(()) => {
                        match self.anvil_manager.start_fork(port, &fork_url2, tx.clone()).await {
                            Ok(()) => {
                                self.anvil.fork_mode = true;
                                self.status_bar.set_message(
                                    format!("Starting fork from {}...", fork_url2),
                                    MessageKind::Info,
                                );
                            }
                            Err(e) => {
                                let _ = self.action_tx.send(Action::AnvilError(e.to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = self.action_tx.send(Action::Error(
                            format!("Fork RPC unreachable: {}", e),
                        ));
                    }
                }
                return;
            }

            // Token actions
            Action::RefreshTokenBalances { ref account } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let account = account.clone();
                let tokens: Vec<_> = self.config.tokens.list.clone();
                self.status_bar.set_message("Loading token balances...".to_string(), MessageKind::Info);
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    // Parallel fetch with futures::join_all
                    let futs: Vec<_> = tokens.iter().map(|token| {
                        let runner_ref = &runner;
                        let account_ref = &account;
                        let addr = token.address.clone();
                        let symbol = token.symbol.clone();
                        let decimals = token.decimals;
                        async move {
                            match runner_ref.token_balance(&addr, account_ref).await {
                                Ok(raw) => {
                                    let formatted = CastRunner::format_token_balance(&raw, decimals);
                                    crate::action::TokenBalance {
                                        token_address: addr,
                                        symbol,
                                        balance: formatted,
                                        raw_balance: raw,
                                        status: crate::action::TokenBalanceStatus::Loaded,
                                    }
                                }
                                Err(e) => {
                                    crate::action::TokenBalance {
                                        token_address: addr,
                                        symbol,
                                        balance: "err".to_string(),
                                        raw_balance: "0".to_string(),
                                        status: crate::action::TokenBalanceStatus::Error(
                                            e.to_string().chars().take(50).collect()
                                        ),
                                    }
                                }
                            }
                        }
                    }).collect();
                    let balances = futures::future::join_all(futs).await;
                    let _ = tx.send(Action::TokenBalancesLoaded { account, balances });
                });
                return;
            }
            Action::TokenBalancesLoaded { .. } => {
                // Delegate to panel
            }
            Action::AddCustomToken { ref address } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let address = address.clone();
                self.status_bar.set_message(
                    format!("Detecting token {}...", address),
                    MessageKind::Info,
                );
                // Try to get a test account for slot detection
                let test_account = self.anvil.accounts().first()
                    .map(|a| a.address.clone());
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    let symbol = runner.token_symbol(&address).await
                        .unwrap_or_else(|_| "???".to_string());
                    let name = runner.token_name(&address).await
                        .unwrap_or_else(|_| "Unknown".to_string());
                    let decimals = runner.token_decimals(&address).await
                        .unwrap_or(18);
                    // Try to auto-detect balance slot
                    let balance_slot = if let Some(ref acct) = test_account {
                        runner.detect_balance_slot(&address, acct).await.ok()
                    } else {
                        // Fall back to well-known slots
                        crate::services::cast_runner::lookup_known_slot(&address)
                    };
                    let entry = crate::config::TokenEntry {
                        address,
                        symbol,
                        name,
                        decimals,
                        balance_slot,
                    };
                    let _ = tx.send(Action::CustomTokenResolved(entry));
                });
                return;
            }
            Action::CustomTokenResolved(ref entry) => {
                self.config.add_token(entry.clone());
                self.anvil.set_tokens(&self.config.tokens.list);
                self.status_bar.set_message(
                    format!("Added token: {} ({})", entry.symbol, entry.name),
                    MessageKind::Success,
                );
                return;
            }
            Action::DealToken { ref token_address, ref to, ref amount, decimals, balance_slot } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let token_address = token_address.clone();
                let to = to.clone();
                let amount = amount.clone();
                let decimals = decimals;
                let balance_slot = balance_slot;
                self.status_bar.set_message("Setting token balance...".to_string(), MessageKind::Info);
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    let raw_amount = CastRunner::compute_raw_amount(&amount, decimals);
                    match runner.deal_token(&token_address, &to, &raw_amount, balance_slot).await {
                        Ok(_) => {
                            let _ = tx.send(Action::DealTokenDone(
                                format!("Deal {} tokens to {}", amount, to),
                            ));
                        }
                        Err(e) => {
                            let _ = tx.send(Action::AnvilError(e.to_string()));
                        }
                    }
                });
                return;
            }
            Action::DealTokenDone(_) => {
                // Delegate to panel
            }
            Action::DealEth { ref to, ref amount } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let to = to.clone();
                let amount = amount.clone();
                self.status_bar.set_message("Setting ETH balance...".to_string(), MessageKind::Info);
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    // Convert ether amount to wei
                    let raw_wei = CastRunner::compute_raw_amount(&amount, 18);
                    match runner.set_eth_balance(&to, &raw_wei).await {
                        Ok(_) => {
                            let _ = tx.send(Action::DealEthDone(
                                format!("Set {} ETH for {}", amount, to),
                            ));
                        }
                        Err(e) => {
                            let _ = tx.send(Action::AnvilError(e.to_string()));
                        }
                    }
                });
                return;
            }
            Action::DealEthDone(_) => {
                // Delegate to panel, then refresh ETH balances
                self.anvil.update(&action);
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let accounts: Vec<_> = self.anvil.accounts().to_vec();
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    let mut updated = accounts;
                    for acc in updated.iter_mut() {
                        if let Ok(bal) = runner.balance(&acc.address).await {
                            acc.balance = bal;
                        }
                    }
                    let _ = tx.send(Action::AnvilAccounts(updated));
                });
                return;
            }
            Action::DetectBalanceSlot { ref token_address, ref test_account } => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let token_address = token_address.clone();
                let test_account = test_account.clone();
                self.status_bar.set_message(
                    format!("Detecting balance slot for {}...", token_address),
                    MessageKind::Info,
                );
                tokio::spawn(async move {
                    let rpc = format!("http://localhost:{}", port);
                    let runner = CastRunner::new(&rpc);
                    match runner.detect_balance_slot(&token_address, &test_account).await {
                        Ok(slot) => {
                            let _ = tx.send(Action::BalanceSlotDetected {
                                token_address,
                                slot,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(Action::Error(
                                format!("Slot detection failed: {}", e),
                            ));
                        }
                    }
                });
                return;
            }
            Action::BalanceSlotDetected { ref token_address, slot } => {
                // Update config with detected slot
                if let Some(entry) = self.config.tokens.list.iter_mut()
                    .find(|t| t.address == *token_address)
                {
                    entry.balance_slot = Some(slot);
                    let _ = self.config.save();
                }
                // Delegate to panel for UI update
            }
            Action::RemoveToken(ref address) => {
                self.config.remove_token(address);
                self.anvil.set_tokens(&self.config.tokens.list);
                self.status_bar.set_message("Token removed".to_string(), MessageKind::Info);
                return;
            }

            // Forge actions
            Action::ForgeBuild => {
                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = ForgeRunner::build(tx.clone()).await {
                        let _ = tx.send(Action::Error(e.to_string()));
                    }
                });
            }
            Action::ForgeTest => {
                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = ForgeRunner::test(tx.clone()).await {
                        let _ = tx.send(Action::Error(e.to_string()));
                    }
                });
            }
            Action::ForgeScript(ref path) => {
                let tx = self.action_tx.clone();
                let path = path.clone();
                tokio::spawn(async move {
                    if let Err(e) = ForgeRunner::script(&path, tx.clone()).await {
                        let _ = tx.send(Action::Error(e.to_string()));
                    }
                });
            }

            // Cast actions
            Action::CastCall { ref to, ref sig, ref args } => {
                let tx = self.action_tx.clone();
                let rpc = self.config.active_rpc_url().to_string();
                let to = to.clone();
                let sig = sig.clone();
                let args = args.clone();
                tokio::spawn(async move {
                    let runner = CastRunner::new(&rpc);
                    match runner.call(&to, &sig, &args).await {
                        Ok(result) => { let _ = tx.send(Action::CastResult(result)); }
                        Err(e) => { let _ = tx.send(Action::CastError(e.to_string())); }
                    }
                });
                return;
            }
            Action::CastSend { ref to, ref sig, ref args } => {
                let tx = self.action_tx.clone();
                let rpc = self.config.active_rpc_url().to_string();
                let to = to.clone();
                let sig = sig.clone();
                let args = args.clone();
                tokio::spawn(async move {
                    let runner = CastRunner::new(&rpc);
                    match runner.send(&to, &sig, &args).await {
                        Ok(result) => { let _ = tx.send(Action::CastResult(result)); }
                        Err(e) => { let _ = tx.send(Action::CastError(e.to_string())); }
                    }
                });
                return;
            }
            Action::CastBalance(ref addr) => {
                let tx = self.action_tx.clone();
                let rpc = self.config.active_rpc_url().to_string();
                let addr = addr.clone();
                tokio::spawn(async move {
                    let runner = CastRunner::new(&rpc);
                    match runner.balance(&addr).await {
                        Ok(result) => { let _ = tx.send(Action::CastResult(result)); }
                        Err(e) => { let _ = tx.send(Action::CastError(e.to_string())); }
                    }
                });
                return;
            }

            // Explorer actions
            Action::RefreshBlocks => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                tokio::spawn(async move {
                    let client = RpcClient::with_port(port);
                    match client.get_recent_blocks(20).await {
                        Ok(blocks) => { let _ = tx.send(Action::BlocksLoaded(blocks)); }
                        Err(e) => { let _ = tx.send(Action::Error(e.to_string())); }
                    }
                });
                return;
            }
            Action::SelectBlock(num) => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let num = num;
                tokio::spawn(async move {
                    let client = RpcClient::with_port(port);
                    match client.get_block_transactions(num).await {
                        Ok(txs) => { let _ = tx.send(Action::TxsLoaded(txs)); }
                        Err(e) => { let _ = tx.send(Action::Error(e.to_string())); }
                    }
                });
            }
            Action::SelectTx(ref hash) => {
                let tx = self.action_tx.clone();
                let port = self.anvil_manager.port();
                let hash = hash.clone();
                tokio::spawn(async move {
                    let client = RpcClient::with_port(port);
                    match client.get_tx_detail(&hash).await {
                        Ok(detail) => { let _ = tx.send(Action::TxDetailLoaded(detail)); }
                        Err(e) => { let _ = tx.send(Action::Error(e.to_string())); }
                    }
                });
            }

            _ => {}
        }

        // Dispatch to active panel for state updates
        let follow_up = match self.active_panel {
            PanelId::Wallets => self.wallets.update(&action),
            PanelId::Anvil => self.anvil.update(&action),
            PanelId::Forge => self.forge.update(&action),
            PanelId::Cast => self.cast.update(&action),
            PanelId::Explorer => self.explorer.update(&action),
        };

        // Also update non-active panels for relevant cross-panel actions
        match action {
            Action::AnvilStarted { .. } | Action::AnvilStopped => {
                self.anvil.update(&action);
            }
            _ => {}
        }

        if let Some(follow_up) = follow_up {
            let _ = self.action_tx.send(follow_up);
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Main layout: sidebar + content + status bar
        let main_chunks = Layout::vertical([
            Constraint::Min(1),   // content area
            Constraint::Length(1), // status bar
        ])
        .split(size);

        let content_chunks = Layout::horizontal([
            Constraint::Length(22), // sidebar
            Constraint::Min(1),    // panel content
        ])
        .split(main_chunks[0]);

        // Draw sidebar
        self.sidebar.draw(frame, content_chunks[0]);

        // Draw active panel
        let panel_area = content_chunks[1];
        match self.active_panel {
            PanelId::Wallets => self.wallets.draw(frame, panel_area),
            PanelId::Anvil => self.anvil.draw(frame, panel_area),
            PanelId::Forge => self.forge.draw(frame, panel_area),
            PanelId::Cast => self.cast.draw(frame, panel_area),
            PanelId::Explorer => self.explorer.draw(frame, panel_area),
        }

        // Sync modal form state for status bar hints
        self.status_bar.anvil_transferring = self.anvil.transferring;
        self.status_bar.anvil_dealing = self.anvil.dealing;
        self.status_bar.anvil_adding_token = self.anvil.adding_token;
        self.status_bar.anvil_editing_fork_url = self.anvil.editing_fork_url;

        // Draw status bar
        self.status_bar.draw(frame, main_chunks[1]);

        // Help overlay
        if self.show_help {
            self.draw_help_overlay(frame, size);
        }
    }

    fn draw_help_overlay(&self, frame: &mut Frame, area: Rect) {
        use crate::theme::Theme;

        let help_w = 50u16.min(area.width.saturating_sub(4));
        let help_h = 30u16.min(area.height.saturating_sub(2));
        let help_area = Rect {
            x: area.x + (area.width.saturating_sub(help_w)) / 2,
            y: area.y + (area.height.saturating_sub(help_h)) / 2,
            width: help_w,
            height: help_h,
        };

        frame.render_widget(Clear, help_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BLUE))
            .title(" Keybindings ")
            .title_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(Theme::BASE));

        let inner = block.inner(help_area);
        frame.render_widget(block, help_area);

        let key_style = Style::default()
            .fg(Theme::BLUE)
            .add_modifier(Modifier::BOLD);
        let desc_style = Style::default().fg(Theme::TEXT);
        let section_style = Style::default()
            .fg(Theme::YELLOW)
            .add_modifier(Modifier::BOLD);

        let mut lines: Vec<Line> = Vec::new();

        // Global
        lines.push(Line::from(Span::styled("-- Global --", section_style)));
        let global_keys: Vec<(&str, &str)> = vec![
            ("h / l", "Switch sub-tabs"),
            ("j / k", "Move up / down"),
            ("Enter", "Select / execute"),
            ("Esc", "Back / sidebar"),
            ("1-5", "Jump to panel"),
            ("q", "Quit"),
            ("?", "This help"),
        ];
        for (k, d) in &global_keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<12}", k), key_style),
                Span::styled(*d, desc_style),
            ]));
        }

        // Panel-specific
        lines.push(Line::from(""));
        let (title, keys): (&str, Vec<(&str, &str)>) = match self.active_panel {
            PanelId::Wallets => ("-- Wallets --", vec![
                ("n", "Create new wallet"),
                ("i", "Import wallet"),
                ("r", "Refresh wallets"),
                ("p", "Unlock (password)"),
            ]),
            PanelId::Anvil => ("-- Anvil --", vec![
                ("s", "Start anvil"),
                ("S", "Stop anvil"),
                ("f", "Fork mainnet"),
                ("F", "Edit fork URL"),
                ("m", "Mine block"),
                ("R", "Reset state"),
                ("t", "Transfer ETH/ERC20"),
                ("D", "Deal token balance"),
                ("E", "Deal ETH balance"),
                ("B", "Detect balance slot"),
                ("a", "Add custom token"),
                ("x", "Remove token"),
                ("r", "Refresh balances"),
                ("d", "Dump state"),
                ("L", "Load state"),
            ]),
            PanelId::Forge => ("-- Forge --", vec![
                ("b", "Build"),
                ("t", "Test"),
                ("c", "Clear output"),
            ]),
            PanelId::Cast => ("-- Cast --", vec![
                ("Enter", "Start editing"),
                ("j / k", "Switch field"),
                ("Esc", "Exit editing"),
            ]),
            PanelId::Explorer => ("-- Explorer --", vec![
                ("r", "Refresh blocks"),
                ("Enter", "View details"),
                ("Esc", "Go back"),
            ]),
        };
        lines.push(Line::from(Span::styled(title, section_style)));
        for (k, d) in &keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<12}", k), key_style),
                Span::styled(*d, desc_style),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Press ? / Esc / q to close",
            Style::default().fg(Theme::OVERLAY0),
        )));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}
