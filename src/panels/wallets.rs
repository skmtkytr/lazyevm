use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, WalletEntry};
use crate::components::Component;
use crate::config::Network;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq)]
enum WalletTab {
    Wallets,
    Networks,
}

pub struct WalletsPanel {
    // Wallets tab
    wallets: Vec<WalletEntry>,
    wallet_state: ListState,
    wallet_selected: usize,
    loading: bool,
    show_detail: bool,
    // Password input for locked wallets
    entering_password: bool,
    password_input: String,

    // Networks tab
    active_tab: WalletTab,
    networks: Vec<Network>,
    active_network: String,
    network_state: ListState,
    network_selected: usize,
    // Add-network form
    adding_network: bool,
    add_name_input: String,
    add_url_input: String,
    add_field: usize, // 0=name, 1=url

    action_tx: Option<UnboundedSender<Action>>,
}

impl WalletsPanel {
    pub fn new() -> Self {
        Self {
            wallets: Vec::new(),
            wallet_state: ListState::default(),
            wallet_selected: 0,
            loading: false,
            show_detail: false,
            entering_password: false,
            password_input: String::new(),

            active_tab: WalletTab::Wallets,
            networks: Vec::new(),
            active_network: String::new(),
            network_state: ListState::default(),
            network_selected: 0,
            adding_network: false,
            add_name_input: String::new(),
            add_url_input: String::new(),
            add_field: 0,

            action_tx: None,
        }
    }

    pub fn set_networks(&mut self, networks: Vec<Network>, active: String) {
        self.networks = networks;
        self.active_network = active;
        if !self.networks.is_empty() && self.network_selected >= self.networks.len() {
            self.network_selected = 0;
            self.network_state.select(Some(0));
        }
    }

    // ── Drawing ──

    fn draw_tab_bar(&self, frame: &mut Frame, area: Rect) {
        let tabs = Tabs::new(vec![Line::from("Wallets"), Line::from("Networks")])
            .select(match self.active_tab {
                WalletTab::Wallets => 0,
                WalletTab::Networks => 1,
            })
            .style(Style::default().fg(Theme::OVERLAY0))
            .highlight_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::styled(" │ ", Style::default().fg(Theme::SURFACE1)));

        frame.render_widget(tabs, area);
    }

    fn draw_wallet_list(&mut self, frame: &mut Frame, area: Rect) {
        if self.wallets.is_empty() && !self.loading {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No wallets found",
                    Style::default().fg(Theme::OVERLAY0),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "n",
                        Style::default()
                            .fg(Theme::BLUE)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" new  ", Style::default().fg(Theme::SUBTEXT0)),
                    Span::styled(
                        "i",
                        Style::default()
                            .fg(Theme::BLUE)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" import", Style::default().fg(Theme::SUBTEXT0)),
                ]),
            ])
            .alignment(ratatui::layout::Alignment::Center);
            frame.render_widget(msg, area);
            return;
        }

        let items: Vec<ListItem> = self
            .wallets
            .iter()
            .enumerate()
            .map(|(i, w)| {
                let addr_display = if w.address.is_empty() {
                    "🔒 locked".to_string()
                } else if w.address.len() > 20 {
                    format!(
                        "{}...{}",
                        &w.address[..10],
                        &w.address[w.address.len() - 8..]
                    )
                } else {
                    w.address.clone()
                };

                let balance_str = match &w.balance {
                    Some(b) if b.starts_with("err:") => "err",
                    Some(b) => b.as_str(),
                    None if w.address.is_empty() => "",
                    None => "...",
                };

                let style = if i == self.wallet_selected {
                    Style::default()
                        .fg(Theme::BLUE)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<14} ", if !w.name.is_empty() { &w.name } else { "—" }),
                        Style::default().fg(Theme::MAUVE),
                    ),
                    Span::styled(format!("{:<24} ", addr_display), style),
                    Span::styled(balance_str, Style::default().fg(Theme::GREEN)),
                ]))
            })
            .collect();

        let title = if self.loading { " loading... " } else { "" };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0))
                    .title(title)
                    .title_style(Style::default().fg(Theme::YELLOW)),
            )
            .highlight_style(Style::default().bg(Theme::SURFACE0));

        frame.render_stateful_widget(list, area, &mut self.wallet_state);
    }

    fn draw_wallet_detail(&self, frame: &mut Frame, area: Rect) {
        let Some(wallet) = self.wallets.get(self.wallet_selected) else {
            return;
        };

        let addr_display = if wallet.address.is_empty() {
            ("(locked — press 'p' to enter password)", Theme::OVERLAY0)
        } else {
            (wallet.address.as_str(), Theme::MAUVE)
        };

        let balance_display = match &wallet.balance {
            Some(b) if b.starts_with("err:") => (b.as_str(), Theme::RED),
            Some(b) => (b.as_str(), Theme::GREEN),
            None if wallet.address.is_empty() => ("—", Theme::OVERLAY0),
            None => ("fetching...", Theme::YELLOW),
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name:    ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled(&wallet.name, Style::default().fg(Theme::TEXT)),
            ]),
            Line::from(vec![
                Span::styled("Address: ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled(addr_display.0, Style::default().fg(addr_display.1)),
            ]),
            Line::from(vec![
                Span::styled("Balance: ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled(balance_display.0, Style::default().fg(balance_display.1)),
            ]),
            Line::from(vec![
                Span::styled("Network: ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled(&self.active_network, Style::default().fg(Theme::SKY)),
            ]),
        ];

        if self.entering_password {
            lines.push(Line::from(""));
            let masked: String = "*".repeat(self.password_input.len());
            lines.push(Line::from(vec![
                Span::styled(
                    "▸ Password: ",
                    Style::default()
                        .fg(Theme::BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(masked, Style::default().fg(Theme::TEXT)),
                Span::styled("█", Style::default().fg(Theme::BLUE)),
            ]));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Theme::SURFACE1))
                    .title(" Detail ")
                    .title_style(Style::default().fg(Theme::SUBTEXT0)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn draw_network_list(&mut self, frame: &mut Frame, area: Rect) {
        if self.adding_network {
            self.draw_add_network_form(frame, area);
            return;
        }

        let items: Vec<ListItem> = self
            .networks
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let is_active = n.name == self.active_network;
                let indicator = if is_active { "● " } else { "  " };
                let style = if i == self.network_selected {
                    Style::default()
                        .fg(Theme::BLUE)
                        .add_modifier(Modifier::BOLD)
                } else if is_active {
                    Style::default().fg(Theme::GREEN)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                let url_short = if n.url.len() > 36 {
                    format!("{}...", &n.url[..36])
                } else {
                    n.url.clone()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        indicator,
                        Style::default().fg(if is_active {
                            Theme::GREEN
                        } else {
                            Theme::SURFACE0
                        }),
                    ),
                    Span::styled(format!("{:<18} ", n.name), style),
                    Span::styled(url_short, Style::default().fg(Theme::SUBTEXT0)),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0))
                    .title(" l:select  n:add  d:delete ")
                    .title_style(Style::default().fg(Theme::OVERLAY0)),
            )
            .highlight_style(Style::default().bg(Theme::SURFACE0));

        frame.render_stateful_widget(list, area, &mut self.network_state);
    }

    fn draw_add_network_form(&self, frame: &mut Frame, area: Rect) {
        let fields = [
            ("Name", &self.add_name_input),
            ("URL ", &self.add_url_input),
        ];

        let mut lines = vec![
            Line::from(Span::styled(
                "Add Network",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        for (i, (label, value)) in fields.iter().enumerate() {
            let is_active = i == self.add_field;
            let indicator = if is_active { "▸ " } else { "  " };
            let label_style = if is_active {
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::OVERLAY0)
            };
            let cursor = if is_active { "█" } else { "" };

            lines.push(Line::from(vec![
                Span::styled(indicator, Style::default().fg(Theme::BLUE)),
                Span::styled(format!("{}: ", label), label_style),
                Span::styled(*value, Style::default().fg(Theme::TEXT)),
                Span::styled(cursor, Style::default().fg(Theme::BLUE)),
            ]));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled("Enter", Style::default().fg(Theme::BLUE)),
            Span::styled(" save  ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("Esc", Style::default().fg(Theme::BLUE)),
            Span::styled(" cancel  ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("j/k", Style::default().fg(Theme::BLUE)),
            Span::styled(" switch field", Style::default().fg(Theme::SUBTEXT0)),
        ]));

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}

impl Component for WalletsPanel {
    fn init(&mut self, action_tx: UnboundedSender<Action>) {
        self.action_tx = Some(action_tx.clone());
        let _ = action_tx.send(Action::RefreshWallets);
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
        // Password input mode
        if self.entering_password {
            match key.code {
                KeyCode::Esc => {
                    self.entering_password = false;
                    self.password_input.clear();
                }
                KeyCode::Enter => {
                    if let Some(w) = self.wallets.get(self.wallet_selected) {
                        let action = Action::UnlockWallet {
                            name: w.name.clone(),
                            password: self.password_input.clone(),
                        };
                        self.entering_password = false;
                        self.password_input.clear();
                        return Some(action);
                    }
                }
                KeyCode::Char(c) => {
                    self.password_input.push(c);
                }
                KeyCode::Backspace => {
                    self.password_input.pop();
                }
                _ => {}
            }
            return Some(Action::None);
        }

        // Add-network form mode
        if self.adding_network {
            match key.code {
                KeyCode::Esc => {
                    self.adding_network = false;
                    self.add_name_input.clear();
                    self.add_url_input.clear();
                }
                KeyCode::Char('j') | KeyCode::Char('k') => {
                    self.add_field = 1 - self.add_field;
                }
                KeyCode::Enter
                    if !self.add_name_input.is_empty() && !self.add_url_input.is_empty() =>
                {
                    let action = Action::AddNetwork {
                        name: self.add_name_input.clone(),
                        url: self.add_url_input.clone(),
                    };
                    self.adding_network = false;
                    self.add_name_input.clear();
                    self.add_url_input.clear();
                    return Some(action);
                }
                KeyCode::Char(c) => {
                    if self.add_field == 0 {
                        self.add_name_input.push(c);
                    } else {
                        self.add_url_input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if self.add_field == 0 {
                        self.add_name_input.pop();
                    } else {
                        self.add_url_input.pop();
                    }
                }
                _ => {}
            }
            return Some(Action::None);
        }

        // Normal mode
        // Back from detail view; if not in detail, let default handle Esc
        if key.code == KeyCode::Esc && self.show_detail {
            return Some(Action::Back);
        }

        match self.active_tab {
            WalletTab::Wallets => match key.code {
                KeyCode::Char('n') => Some(Action::CreateWallet),
                KeyCode::Char('i') => Some(Action::ImportWallet),
                KeyCode::Char('r') => Some(Action::RefreshWallets),
                KeyCode::Char('p') => {
                    if let Some(w) = self.wallets.get(self.wallet_selected) {
                        if w.address.is_empty() {
                            self.entering_password = true;
                            self.show_detail = true;
                            return Some(Action::None);
                        }
                    }
                    None
                }
                _ => None,
            },
            WalletTab::Networks => match key.code {
                KeyCode::Char('n') => {
                    self.adding_network = true;
                    self.add_field = 0;
                    self.add_name_input.clear();
                    self.add_url_input.clear();
                    Some(Action::None)
                }
                KeyCode::Char('d') => {
                    if let Some(net) = self.networks.get(self.network_selected) {
                        let name = net.name.clone();
                        Some(Action::RemoveNetwork(name))
                    } else {
                        None
                    }
                }
                _ => None,
            },
        }
    }

    fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::WalletsLoaded(wallets) => {
                self.wallets = wallets.clone();
                self.loading = false;
                if !self.wallets.is_empty() {
                    self.wallet_state.select(Some(0));
                    self.wallet_selected = 0;
                }
            }
            Action::RefreshWallets => {
                self.loading = true;
            }
            Action::BalanceLoaded { address, balance } => {
                if let Some(w) = self.wallets.iter_mut().find(|w| w.address == *address) {
                    w.balance = Some(balance.clone());
                }
            }
            Action::WalletCreated(msg) | Action::WalletImported(msg) => {
                return Some(Action::SetStatus(msg.clone()));
            }
            Action::WalletAddressResolved { name, address } => {
                if let Some(w) = self.wallets.iter_mut().find(|w| w.name == *name) {
                    w.address = address.clone();
                }
            }
            Action::NetworkChanged { name, .. } => {
                self.active_network = name.clone();
                // Clear all balances so they re-fetch with new network
                for w in &mut self.wallets {
                    if !w.address.is_empty() {
                        w.balance = None;
                    }
                }
                return Some(Action::RefreshWallets);
            }
            Action::Select => match self.active_tab {
                WalletTab::Wallets => {
                    if !self.wallets.is_empty() {
                        self.show_detail = !self.show_detail;
                    }
                }
                WalletTab::Networks => {
                    if let Some(net) = self.networks.get(self.network_selected) {
                        return Some(Action::SelectNetwork(net.name.clone()));
                    }
                }
            },
            Action::Back if self.show_detail => {
                self.show_detail = false;
            }
            Action::Up => match self.active_tab {
                WalletTab::Wallets => {
                    if !self.wallets.is_empty() {
                        self.wallet_selected = self.wallet_selected.saturating_sub(1);
                        self.wallet_state.select(Some(self.wallet_selected));
                    }
                }
                WalletTab::Networks => {
                    if !self.networks.is_empty() {
                        self.network_selected = self.network_selected.saturating_sub(1);
                        self.network_state.select(Some(self.network_selected));
                    }
                }
            },
            Action::Down => match self.active_tab {
                WalletTab::Wallets => {
                    if !self.wallets.is_empty() {
                        self.wallet_selected =
                            (self.wallet_selected + 1).min(self.wallets.len() - 1);
                        self.wallet_state.select(Some(self.wallet_selected));
                    }
                }
                WalletTab::Networks => {
                    if !self.networks.is_empty() {
                        self.network_selected =
                            (self.network_selected + 1).min(self.networks.len() - 1);
                        self.network_state.select(Some(self.network_selected));
                    }
                }
            },
            Action::NextTab => {
                self.active_tab = match self.active_tab {
                    WalletTab::Wallets => WalletTab::Networks,
                    WalletTab::Networks => WalletTab::Wallets,
                };
            }
            Action::PrevTab => {
                if self.active_tab == WalletTab::Wallets {
                    return Some(Action::FocusSidebar);
                }
                self.active_tab = WalletTab::Wallets;
            }
            _ => {}
        }
        None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let net_label = if self.active_network.is_empty() {
            String::new()
        } else {
            format!(" [{}]", self.active_network)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BLUE))
            .title(format!(" Wallets{} ", net_label))
            .title_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1), // tab bar
            Constraint::Min(1),    // content
        ])
        .split(inner);

        self.draw_tab_bar(frame, chunks[0]);

        match self.active_tab {
            WalletTab::Wallets => {
                if self.show_detail {
                    let split =
                        Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)])
                            .split(chunks[1]);
                    self.draw_wallet_list(frame, split[0]);
                    self.draw_wallet_detail(frame, split[1]);
                } else {
                    self.draw_wallet_list(frame, chunks[1]);
                }
            }
            WalletTab::Networks => {
                self.draw_network_list(frame, chunks[1]);
            }
        }
    }
}
