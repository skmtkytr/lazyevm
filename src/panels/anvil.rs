use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, AnvilAccount, TokenBalance, TokenBalanceStatus};
use crate::components::Component;
use crate::config::TokenEntry;
use crate::services::cast_runner::CastRunner;
use crate::theme::Theme;

/// Max number of log lines to keep
const MAX_LOG_LINES: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AnvilTab {
    Accounts,
    Tokens,
    Config,
    Logs,
}

impl AnvilTab {
    fn all() -> Vec<Self> {
        vec![Self::Accounts, Self::Tokens, Self::Config, Self::Logs]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Accounts => "Accounts",
            Self::Tokens => "Tokens",
            Self::Config => "Config",
            Self::Logs => "Logs",
        }
    }

    fn index(&self) -> usize {
        match self {
            Self::Accounts => 0,
            Self::Tokens => 1,
            Self::Config => 2,
            Self::Logs => 3,
        }
    }

    fn next(&self) -> Self {
        match self {
            Self::Accounts => Self::Tokens,
            Self::Tokens => Self::Config,
            Self::Config => Self::Logs,
            Self::Logs => Self::Accounts,
        }
    }

    fn prev(&self) -> Option<Self> {
        match self {
            Self::Accounts => None, // signal to go to sidebar
            Self::Tokens => Some(Self::Accounts),
            Self::Config => Some(Self::Tokens),
            Self::Logs => Some(Self::Config),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TokenDisplayEntry {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub balance_slot: Option<u64>,
    pub balance: String,
    pub raw_balance: String,
    pub status: TokenBalanceStatus,
}

pub struct AnvilPanel {
    pub running: bool,
    pub port: u16,
    pub block_number: u64,
    accounts: Vec<AnvilAccount>,
    logs: Vec<String>,
    active_tab: AnvilTab,
    list_state: ListState,
    selected: usize,
    log_scroll: u16,
    action_tx: Option<UnboundedSender<Action>>,
    // Transfer form
    pub transferring: bool,
    transfer_from_key: String,
    transfer_token: String,
    transfer_to: String,
    transfer_amount: String,
    transfer_field: usize, // 0=token, 1=to, 2=amount
    // Fork mode
    pub fork_mode: bool,
    fork_url: String,
    // Tokens
    tokens: Vec<TokenDisplayEntry>,
    token_selected: usize,
    token_list_state: ListState,
    // Deal form
    pub dealing: bool,
    deal_amount: String,
    // Add token form
    pub adding_token: bool,
    add_token_address: String,
    // Edit fork URL form
    pub editing_fork_url: bool,
    edit_fork_url_buf: String,
}

impl AnvilPanel {
    pub fn new() -> Self {
        Self {
            running: false,
            port: 8545,
            block_number: 0,
            accounts: Vec::new(),
            logs: Vec::new(),
            active_tab: AnvilTab::Accounts,
            list_state: ListState::default(),
            selected: 0,
            log_scroll: 0,
            action_tx: None,
            transferring: false,
            transfer_from_key: String::new(),
            transfer_token: String::new(),
            transfer_to: String::new(),
            transfer_amount: String::new(),
            transfer_field: 0,
            fork_mode: false,
            fork_url: String::new(),
            tokens: Vec::new(),
            token_selected: 0,
            token_list_state: ListState::default(),
            dealing: false,
            deal_amount: String::new(),
            adding_token: false,
            add_token_address: String::new(),
            editing_fork_url: false,
            edit_fork_url_buf: String::new(),
        }
    }

    pub fn accounts(&self) -> &[AnvilAccount] {
        &self.accounts
    }

    pub fn set_tokens(&mut self, entries: &[TokenEntry]) {
        self.tokens = entries
            .iter()
            .map(|e| TokenDisplayEntry {
                address: e.address.clone(),
                symbol: e.symbol.clone(),
                name: e.name.clone(),
                decimals: e.decimals,
                balance_slot: e.balance_slot,
                balance: "-".to_string(),
                raw_balance: "0".to_string(),
                status: TokenBalanceStatus::Unknown,
            })
            .collect();
    }

    pub fn set_fork_url(&mut self, url: String) {
        self.fork_url = url;
    }

    pub fn update_token_balances(&mut self, balances: &[TokenBalance]) {
        for bal in balances {
            if let Some(token) = self
                .tokens
                .iter_mut()
                .find(|t| t.address == bal.token_address)
            {
                token.balance = bal.balance.clone();
                token.raw_balance = bal.raw_balance.clone();
                token.status = bal.status.clone();
            }
        }
    }

    /// Mark all tokens as loading
    fn mark_tokens_loading(&mut self) {
        for t in &mut self.tokens {
            t.status = TokenBalanceStatus::Loading;
        }
    }

    /// Update a token's balance_slot after detection
    pub fn update_token_slot(&mut self, address: &str, slot: u64) {
        if let Some(token) = self.tokens.iter_mut().find(|t| t.address == address) {
            token.balance_slot = Some(slot);
        }
    }

    fn selected_account_address(&self) -> Option<String> {
        self.accounts.get(self.selected).map(|a| a.address.clone())
    }

    fn fire_refresh_tokens(&mut self) {
        let addr = self.selected_account_address();
        if let (Some(tx), Some(addr)) = (self.action_tx.clone(), addr) {
            self.mark_tokens_loading();
            let _ = tx.send(Action::RefreshTokenBalances { account: addr });
        }
    }

    fn draw_status(&self, frame: &mut Frame, area: Rect) {
        let status_color = if self.running {
            Theme::GREEN
        } else {
            Theme::RED
        };
        let status_text = if self.running { "Running" } else { "Stopped" };

        let mut spans = vec![
            Span::styled("Status: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                status_text,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled("Port: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(self.port.to_string(), Style::default().fg(Theme::TEXT)),
            Span::raw("  "),
            Span::styled("Block: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                format!("#{}", self.block_number),
                Style::default().fg(Theme::YELLOW),
            ),
        ];

        if self.fork_mode {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                "FORK",
                Style::default()
                    .fg(Theme::MAUVE)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let lines = vec![Line::from(spans)];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Theme::SURFACE0)),
        );

        frame.render_widget(paragraph, area);
    }

    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        let tabs: Vec<Line> = AnvilTab::all()
            .iter()
            .map(|t| Line::from(t.label()))
            .collect();

        let tabs_widget = Tabs::new(tabs)
            .select(self.active_tab.index())
            .style(Style::default().fg(Theme::OVERLAY0))
            .highlight_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::styled(" | ", Style::default().fg(Theme::SURFACE1)));

        frame.render_widget(tabs_widget, area);
    }

    fn draw_accounts(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .accounts
            .iter()
            .enumerate()
            .map(|(i, acc)| {
                let addr_short = if acc.address.len() > 20 {
                    format!(
                        "{}...{}",
                        &acc.address[..10],
                        &acc.address[acc.address.len() - 8..]
                    )
                } else {
                    acc.address.clone()
                };

                let style = if i == self.selected {
                    Style::default().fg(Theme::BLUE)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("[{}] ", i), Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(format!("{:<22} ", addr_short), style),
                    Span::styled(&acc.balance, Style::default().fg(Theme::GREEN)),
                ]))
            })
            .collect();

        let list = List::new(items).highlight_style(Style::default().bg(Theme::SURFACE0));

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn draw_tokens(&mut self, frame: &mut Frame, area: Rect) {
        if !self.fork_mode {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Tokens tab requires Fork mode.",
                    Style::default().fg(Theme::OVERLAY0),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press 'f' to start Anvil in fork mode.",
                    Style::default().fg(Theme::OVERLAY0),
                )),
            ]);
            frame.render_widget(msg, area);
            return;
        }

        if self.tokens.is_empty() {
            let msg = Paragraph::new(Line::from(Span::styled(
                "  No tokens configured. Press 'a' to add one.",
                Style::default().fg(Theme::OVERLAY0),
            )));
            frame.render_widget(msg, area);
            return;
        }

        let items: Vec<ListItem> = self
            .tokens
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let style = if i == self.token_selected {
                    Style::default().fg(Theme::BLUE)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                let addr_short = if t.address.len() > 14 {
                    format!(
                        "{}...{}",
                        &t.address[..6],
                        &t.address[t.address.len() - 4..]
                    )
                } else {
                    t.address.clone()
                };

                let (balance_text, balance_color) = match &t.status {
                    TokenBalanceStatus::Loading => ("loading...".to_string(), Theme::YELLOW),
                    TokenBalanceStatus::Error(e) => {
                        let short = if e.len() > 10 { &e[..10] } else { e };
                        (format!("err:{}", short), Theme::RED)
                    }
                    TokenBalanceStatus::Loaded => (t.balance.clone(), Theme::GREEN),
                    TokenBalanceStatus::Unknown => ("-".to_string(), Theme::OVERLAY0),
                };

                let slot_indicator = if t.balance_slot.is_some() {
                    ""
                } else {
                    " [no slot]"
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{:<6} ", t.symbol),
                        style.add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{:<14} ", balance_text),
                        Style::default().fg(balance_color),
                    ),
                    Span::styled(addr_short, Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(slot_indicator, Style::default().fg(Theme::RED)),
                ]))
            })
            .collect();

        let list = List::new(items).highlight_style(Style::default().bg(Theme::SURFACE0));

        frame.render_stateful_widget(list, area, &mut self.token_list_state);
    }

    fn draw_config(&self, frame: &mut Frame, area: Rect) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("RPC URL:     ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled(
                    format!("http://localhost:{}", self.port),
                    Style::default().fg(Theme::TEXT),
                ),
            ]),
            Line::from(vec![
                Span::styled("Chain ID:    ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled("31337", Style::default().fg(Theme::TEXT)),
            ]),
            Line::from(vec![
                Span::styled("Gas Limit:   ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled("30000000", Style::default().fg(Theme::TEXT)),
            ]),
            Line::from(vec![
                Span::styled("Gas Price:   ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled("0", Style::default().fg(Theme::TEXT)),
            ]),
            Line::from(vec![
                Span::styled("Block Time:  ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled("auto", Style::default().fg(Theme::TEXT)),
            ]),
        ];

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Fork Mode:   ", Style::default().fg(Theme::OVERLAY0)),
            if self.fork_mode {
                Span::styled(
                    "ON",
                    Style::default()
                        .fg(Theme::GREEN)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("OFF", Style::default().fg(Theme::RED))
            },
        ]));
        if !self.fork_url.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Fork URL:    ", Style::default().fg(Theme::OVERLAY0)),
                Span::styled(&self.fork_url, Style::default().fg(Theme::TEXT)),
            ]));
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    fn draw_transfer_form(&self, frame: &mut Frame, area: Rect) {
        let title = if self.transfer_token.is_empty() {
            " Transfer ETH "
        } else {
            " Transfer ERC20 "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::YELLOW))
            .title(title)
            .title_style(
                Style::default()
                    .fg(Theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1), // Token label + input
            Constraint::Length(1), // To label + input
            Constraint::Length(1), // Amount label + input
            Constraint::Length(1), // spacer
            Constraint::Length(1), // help
        ])
        .split(inner);

        let field_style = |idx: usize| -> Style {
            if self.transfer_field == idx {
                Style::default()
                    .fg(Theme::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::TEXT)
            }
        };

        let cursor = |idx: usize| -> &str {
            if self.transfer_field == idx {
                "\u{2588}"
            } else {
                ""
            }
        };

        let token_line = Line::from(vec![
            Span::styled("Token: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                format!("{}{}", self.transfer_token, cursor(0)),
                field_style(0),
            ),
            if self.transfer_token.is_empty() && self.transfer_field != 0 {
                Span::styled(" (empty=ETH)", Style::default().fg(Theme::OVERLAY0))
            } else {
                Span::raw("")
            },
        ]);
        frame.render_widget(Paragraph::new(token_line), chunks[0]);

        let to_line = Line::from(vec![
            Span::styled("   To: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(format!("{}{}", self.transfer_to, cursor(1)), field_style(1)),
        ]);
        frame.render_widget(Paragraph::new(to_line), chunks[1]);

        let amount_suffix = if self.transfer_token.is_empty() {
            " ETH"
        } else {
            " (raw)"
        };
        let amount_line = Line::from(vec![
            Span::styled("  Amt: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                format!("{}{}{}", self.transfer_amount, cursor(2), amount_suffix),
                field_style(2),
            ),
        ]);
        frame.render_widget(Paragraph::new(amount_line), chunks[2]);

        let help = Line::from(vec![
            Span::styled(
                " j/k",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" switch ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" send ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Theme::SUBTEXT0)),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[4]);
    }

    fn draw_deal_form(&self, frame: &mut Frame, area: Rect) {
        let is_eth_mode = self.active_tab != AnvilTab::Tokens;
        let token_symbol = if is_eth_mode {
            "ETH"
        } else {
            self.tokens
                .get(self.token_selected)
                .map(|t| t.symbol.as_str())
                .unwrap_or("?")
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::MAUVE))
            .title(format!(" Deal {} ", token_symbol))
            .title_style(
                Style::default()
                    .fg(Theme::MAUVE)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1), // Amount
            Constraint::Length(1), // spacer
            Constraint::Length(1), // help
        ])
        .split(inner);

        let amount_line = Line::from(vec![
            Span::styled("Amount: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                format!("{}\u{2588}", self.deal_amount),
                Style::default()
                    .fg(Theme::MAUVE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", token_symbol),
                Style::default().fg(Theme::OVERLAY0),
            ),
        ]);
        frame.render_widget(Paragraph::new(amount_line), chunks[0]);

        let help = Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" deal ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Theme::SUBTEXT0)),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[2]);
    }

    fn draw_add_token_form(&self, frame: &mut Frame, area: Rect) {
        let is_valid = CastRunner::is_valid_address(&self.add_token_address);
        let border_color = if !self.add_token_address.is_empty() && !is_valid {
            Theme::YELLOW
        } else {
            Theme::GREEN
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(" Add Token ")
            .title_style(
                Style::default()
                    .fg(Theme::GREEN)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1), // Address
            Constraint::Length(1), // validation hint
            Constraint::Length(1), // help
        ])
        .split(inner);

        let addr_line = Line::from(vec![
            Span::styled("Address: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                format!("{}\u{2588}", self.add_token_address),
                Style::default()
                    .fg(Theme::GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(addr_line), chunks[0]);

        // Validation hint
        if !self.add_token_address.is_empty() && !is_valid {
            let hint = Line::from(Span::styled(
                format!(" {}/42 chars", self.add_token_address.len()),
                Style::default().fg(Theme::YELLOW),
            ));
            frame.render_widget(Paragraph::new(hint), chunks[1]);
        }

        let help = Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" detect ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Theme::SUBTEXT0)),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[2]);
    }

    fn draw_edit_fork_url_form(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::SKY))
            .title(" Edit Fork URL ")
            .title_style(Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1), // URL input
            Constraint::Length(1), // hint
            Constraint::Length(1), // help
        ])
        .split(inner);

        let url_line = Line::from(vec![
            Span::styled("URL: ", Style::default().fg(Theme::OVERLAY0)),
            Span::styled(
                format!("{}\u{2588}", self.edit_fork_url_buf),
                Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD),
            ),
        ]);
        frame.render_widget(Paragraph::new(url_line), chunks[0]);

        let hint = Line::from(Span::styled(
            " e.g. https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY (empty to clear)",
            Style::default().fg(Theme::OVERLAY0),
        ));
        frame.render_widget(Paragraph::new(hint), chunks[1]);

        let help = Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" save ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Theme::SUBTEXT0)),
        ]);
        frame.render_widget(Paragraph::new(help), chunks[2]);
    }

    fn draw_logs(&self, frame: &mut Frame, area: Rect) {
        let lines: Vec<Line> = self
            .logs
            .iter()
            .map(|l| {
                let color = if l.contains("Error") || l.contains("error") {
                    Theme::RED
                } else if l.contains("Warning") || l.contains("warn") {
                    Theme::YELLOW
                } else {
                    Theme::SUBTEXT0
                };
                Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
            })
            .collect();

        let paragraph = Paragraph::new(lines)
            .scroll((self.log_scroll, 0))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}

impl Component for AnvilPanel {
    fn init(&mut self, action_tx: UnboundedSender<Action>) {
        self.action_tx = Some(action_tx);
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
        // Deal form mode
        if self.dealing {
            match key.code {
                KeyCode::Esc => {
                    self.dealing = false;
                    return Some(Action::None);
                }
                KeyCode::Enter => {
                    if self.deal_amount.is_empty() {
                        return Some(Action::None);
                    }
                    if let Some(account) = self.selected_account_address() {
                        // If on Tokens tab and have a selected token, deal ERC20
                        if self.active_tab == AnvilTab::Tokens {
                            if let Some(token) = self.tokens.get(self.token_selected) {
                                if let Some(slot) = token.balance_slot {
                                    let action = Action::DealToken {
                                        token_address: token.address.clone(),
                                        to: account,
                                        amount: self.deal_amount.clone(),
                                        decimals: token.decimals,
                                        balance_slot: slot,
                                    };
                                    self.dealing = false;
                                    return Some(action);
                                } else {
                                    self.dealing = false;
                                    return Some(Action::Error(
                                        "Token has no balance_slot. Press 'B' to detect."
                                            .to_string(),
                                    ));
                                }
                            }
                        }
                        // Otherwise, deal ETH
                        let action = Action::DealEth {
                            to: account,
                            amount: self.deal_amount.clone(),
                        };
                        self.dealing = false;
                        return Some(action);
                    }
                    self.dealing = false;
                    return Some(Action::None);
                }
                KeyCode::Char(c) => {
                    if c.is_ascii_digit() || (c == '.' && !self.deal_amount.contains('.')) {
                        self.deal_amount.push(c);
                    }
                    return Some(Action::None);
                }
                KeyCode::Backspace => {
                    self.deal_amount.pop();
                    return Some(Action::None);
                }
                _ => return Some(Action::None),
            }
        }

        // Add token form mode
        if self.adding_token {
            match key.code {
                KeyCode::Esc => {
                    self.adding_token = false;
                    return Some(Action::None);
                }
                KeyCode::Enter => {
                    if self.add_token_address.is_empty() {
                        return Some(Action::None);
                    }
                    if !CastRunner::is_valid_address(&self.add_token_address) {
                        return Some(Action::Error(
                            "Invalid address (need 0x + 40 hex chars)".to_string(),
                        ));
                    }
                    let action = Action::AddCustomToken {
                        address: self.add_token_address.clone(),
                    };
                    self.adding_token = false;
                    return Some(action);
                }
                KeyCode::Char(c) => {
                    if c.is_ascii_hexdigit() || c == 'x' || c == 'X' {
                        self.add_token_address.push(c);
                    }
                    return Some(Action::None);
                }
                KeyCode::Backspace => {
                    self.add_token_address.pop();
                    return Some(Action::None);
                }
                _ => return Some(Action::None),
            }
        }

        // Edit fork URL form mode
        if self.editing_fork_url {
            match key.code {
                KeyCode::Esc => {
                    self.editing_fork_url = false;
                    return Some(Action::None);
                }
                KeyCode::Enter => {
                    let url = self.edit_fork_url_buf.trim().to_string();
                    self.editing_fork_url = false;
                    if url.is_empty() {
                        // Clear fork URL
                        self.fork_url.clear();
                        return Some(Action::SetForkUrl(String::new()));
                    }
                    if !url.starts_with("http://")
                        && !url.starts_with("https://")
                        && !url.starts_with("ws://")
                        && !url.starts_with("wss://")
                    {
                        return Some(Action::Error(
                            "URL must start with http://, https://, ws://, or wss://".to_string(),
                        ));
                    }
                    self.fork_url = url.clone();
                    return Some(Action::SetForkUrl(url));
                }
                KeyCode::Char(c) => {
                    self.edit_fork_url_buf.push(c);
                    return Some(Action::None);
                }
                KeyCode::Backspace => {
                    self.edit_fork_url_buf.pop();
                    return Some(Action::None);
                }
                _ => return Some(Action::None),
            }
        }

        // Transfer form mode
        if self.transferring {
            match key.code {
                KeyCode::Esc => {
                    self.transferring = false;
                    return Some(Action::None);
                }
                KeyCode::Char('j') => {
                    self.transfer_field = (self.transfer_field + 1) % 3;
                    return Some(Action::None);
                }
                KeyCode::Char('k') => {
                    self.transfer_field = (self.transfer_field + 2) % 3;
                    return Some(Action::None);
                }
                KeyCode::Tab => {
                    self.transfer_field = (self.transfer_field + 1) % 3;
                    return Some(Action::None);
                }
                KeyCode::Enter => {
                    if self.transfer_to.is_empty() || self.transfer_amount.is_empty() {
                        return Some(Action::None);
                    }
                    let token = if self.transfer_token.is_empty() {
                        None
                    } else {
                        Some(self.transfer_token.clone())
                    };
                    let action = Action::AnvilTransfer {
                        from_key: self.transfer_from_key.clone(),
                        to: self.transfer_to.clone(),
                        value: self.transfer_amount.clone(),
                        token,
                    };
                    self.transferring = false;
                    return Some(action);
                }
                KeyCode::Char(c) => {
                    match self.transfer_field {
                        0 if c.is_ascii_hexdigit() || c == 'x' || c == 'X' => {
                            self.transfer_token.push(c);
                        }
                        1 => {
                            self.transfer_to.push(c);
                        }
                        2 if c.is_ascii_digit() || c == '.' => {
                            self.transfer_amount.push(c);
                        }
                        _ => {}
                    }
                    return Some(Action::None);
                }
                KeyCode::Backspace => {
                    match self.transfer_field {
                        0 => {
                            self.transfer_token.pop();
                        }
                        1 => {
                            self.transfer_to.pop();
                        }
                        2 => {
                            self.transfer_amount.pop();
                        }
                        _ => {}
                    }
                    return Some(Action::None);
                }
                _ => return Some(Action::None),
            }
        }

        match key.code {
            KeyCode::Char('s') => {
                if !self.running {
                    Some(Action::StartAnvil)
                } else {
                    None
                }
            }
            KeyCode::Char('S') => {
                if self.running {
                    Some(Action::StopAnvil)
                } else {
                    None
                }
            }
            KeyCode::Char('f') => {
                if !self.running && !self.fork_url.is_empty() {
                    Some(Action::StartAnvilFork {
                        fork_url: self.fork_url.clone(),
                    })
                } else {
                    None
                }
            }
            KeyCode::Char('F') => {
                if !self.running {
                    self.editing_fork_url = true;
                    self.edit_fork_url_buf = self.fork_url.clone();
                    Some(Action::None)
                } else {
                    None
                }
            }
            KeyCode::Char('m') => {
                if self.running {
                    Some(Action::MineBlock)
                } else {
                    None
                }
            }
            KeyCode::Char('R') => {
                if self.running {
                    Some(Action::ResetAnvil)
                } else {
                    None
                }
            }
            KeyCode::Char('t') => {
                if self.running
                    && self.active_tab == AnvilTab::Accounts
                    && !self.accounts.is_empty()
                {
                    self.transferring = true;
                    self.transfer_from_key = self.accounts[self.selected].key.clone();
                    self.transfer_token.clear();
                    self.transfer_to.clear();
                    self.transfer_amount.clear();
                    self.transfer_field = 0;
                    Some(Action::None)
                } else {
                    None
                }
            }
            KeyCode::Char('D') => {
                if self.running
                    && self.fork_mode
                    && self.active_tab == AnvilTab::Tokens
                    && !self.tokens.is_empty()
                    && !self.accounts.is_empty()
                {
                    // Pre-check: token must have a balance_slot
                    if let Some(token) = self.tokens.get(self.token_selected) {
                        if token.balance_slot.is_none() {
                            return Some(Action::Error(format!(
                                "Token {} has no balance_slot. Press 'B' to auto-detect.",
                                token.symbol
                            )));
                        }
                    }
                    self.dealing = true;
                    self.deal_amount.clear();
                    Some(Action::None)
                } else {
                    None
                }
            }
            KeyCode::Char('E') => {
                // ETH deal
                if self.running && self.fork_mode && !self.accounts.is_empty() {
                    self.dealing = true;
                    self.deal_amount.clear();
                    // Use a sentinel to indicate ETH deal mode in deal form
                    // We'll handle this by checking if we're on Tokens tab or not
                    Some(Action::None)
                } else {
                    None
                }
            }
            KeyCode::Char('B') => {
                // Auto-detect balance slot
                if self.running
                    && self.fork_mode
                    && self.active_tab == AnvilTab::Tokens
                    && !self.tokens.is_empty()
                {
                    if let (Some(token), Some(account)) = (
                        self.tokens.get(self.token_selected).cloned(),
                        self.selected_account_address(),
                    ) {
                        return Some(Action::DetectBalanceSlot {
                            token_address: token.address,
                            test_account: account,
                        });
                    }
                }
                None
            }
            KeyCode::Char('a') => {
                if self.running && self.fork_mode && self.active_tab == AnvilTab::Tokens {
                    self.adding_token = true;
                    self.add_token_address.clear();
                    Some(Action::None)
                } else {
                    None
                }
            }
            KeyCode::Char('x') => {
                if self.active_tab == AnvilTab::Tokens && !self.tokens.is_empty() {
                    let addr = self.tokens[self.token_selected].address.clone();
                    Some(Action::RemoveToken(addr))
                } else {
                    None
                }
            }
            KeyCode::Char('r') => {
                if self.running && self.active_tab == AnvilTab::Tokens {
                    self.fire_refresh_tokens();
                    Some(Action::None)
                } else {
                    None
                }
            }
            KeyCode::Char('d') => {
                if self.running {
                    Some(Action::AnvilDumpState)
                } else {
                    None
                }
            }
            KeyCode::Char('L') => {
                if self.running {
                    Some(Action::AnvilLoadState)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::AnvilStarted { port } => {
                self.running = true;
                self.port = *port;
                self.logs.clear();
                self.logs.push(format!("Anvil started on port {}", port));
                return Some(Action::SetStatus(format!("Anvil started on port {}", port)));
            }
            Action::AnvilStopped => {
                self.running = false;
                self.fork_mode = false;
                self.accounts.clear();
                self.block_number = 0;
                self.logs.push("Anvil stopped".to_string());
                // Reset token balances
                for t in &mut self.tokens {
                    t.balance = "-".to_string();
                    t.raw_balance = "0".to_string();
                    t.status = TokenBalanceStatus::Unknown;
                }
                return Some(Action::SetStatus("Anvil stopped".to_string()));
            }
            Action::AnvilLog(log) => {
                self.logs.push(log.clone());
                // Circular buffer: cap at MAX_LOG_LINES
                if self.logs.len() > MAX_LOG_LINES {
                    let drain = self.logs.len() - MAX_LOG_LINES;
                    self.logs.drain(..drain);
                }
                if self.logs.len() > 1 {
                    self.log_scroll = self.logs.len().saturating_sub(1) as u16;
                }
            }
            Action::AnvilAccounts(accounts) => {
                self.accounts = accounts.clone();
                if !self.accounts.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            Action::BlockMined(num) => {
                self.block_number = *num;
                self.logs.push(format!("Block #{} mined", num));
            }
            Action::AnvilTransferDone(ref msg) => {
                self.logs.push(format!("Transfer: {}", msg));
                if let Some(ref tx) = self.action_tx {
                    let _ = tx.send(Action::SetStatus(msg.clone()));
                }
            }
            Action::TokenBalancesLoaded { balances, .. } => {
                self.update_token_balances(balances);
            }
            Action::DealTokenDone(ref msg) => {
                self.logs.push(format!("Deal: {}", msg));
                self.fire_refresh_tokens();
                return Some(Action::SetStatus(msg.clone()));
            }
            Action::DealEthDone(ref msg) => {
                self.logs.push(format!("ETH Deal: {}", msg));
                return Some(Action::SetStatus(msg.clone()));
            }
            Action::BalanceSlotDetected {
                ref token_address,
                slot,
            } => {
                self.update_token_slot(token_address, *slot);
                self.logs
                    .push(format!("Detected slot {} for {}", slot, token_address));
                return Some(Action::SetStatus(format!("Balance slot {} detected", slot)));
            }
            Action::AnvilError(err) => {
                self.logs.push(format!("Error: {}", err));
                return Some(Action::Error(err.clone()));
            }
            Action::Up => match self.active_tab {
                AnvilTab::Accounts if !self.accounts.is_empty() => {
                    let prev_selected = self.selected;
                    self.selected = self.selected.saturating_sub(1);
                    self.list_state.select(Some(self.selected));
                    if self.fork_mode && prev_selected != self.selected {
                        self.fire_refresh_tokens();
                    }
                }
                AnvilTab::Tokens if !self.tokens.is_empty() => {
                    self.token_selected = self.token_selected.saturating_sub(1);
                    self.token_list_state.select(Some(self.token_selected));
                }
                AnvilTab::Logs => {
                    self.log_scroll = self.log_scroll.saturating_sub(1);
                }
                _ => {}
            },
            Action::Down => match self.active_tab {
                AnvilTab::Accounts if !self.accounts.is_empty() => {
                    let prev_selected = self.selected;
                    self.selected = (self.selected + 1).min(self.accounts.len() - 1);
                    self.list_state.select(Some(self.selected));
                    if self.fork_mode && prev_selected != self.selected {
                        self.fire_refresh_tokens();
                    }
                }
                AnvilTab::Tokens if !self.tokens.is_empty() => {
                    self.token_selected = (self.token_selected + 1).min(self.tokens.len() - 1);
                    self.token_list_state.select(Some(self.token_selected));
                }
                AnvilTab::Logs => {
                    self.log_scroll += 1;
                }
                _ => {}
            },
            Action::NextTab => {
                let prev = self.active_tab;
                self.active_tab = self.active_tab.next();
                if self.active_tab == AnvilTab::Tokens && self.fork_mode && prev != AnvilTab::Tokens
                {
                    self.fire_refresh_tokens();
                }
            }
            Action::PrevTab => match self.active_tab.prev() {
                None => return Some(Action::FocusSidebar),
                Some(tab) => self.active_tab = tab,
            },
            _ => {}
        }
        None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BLUE))
            .title(" Anvil ")
            .title_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(2), // status
            Constraint::Length(1), // tabs
            Constraint::Min(1),    // content
        ])
        .split(inner);

        self.draw_status(frame, chunks[0]);
        self.draw_tabs(frame, chunks[1]);

        match self.active_tab {
            AnvilTab::Accounts => self.draw_accounts(frame, chunks[2]),
            AnvilTab::Tokens => self.draw_tokens(frame, chunks[2]),
            AnvilTab::Config => self.draw_config(frame, chunks[2]),
            AnvilTab::Logs => self.draw_logs(frame, chunks[2]),
        }

        // Transfer form overlay
        if self.transferring {
            let form_height = 7u16;
            let form_width = 60u16.min(inner.width);
            let form_area = Rect {
                x: inner.x + (inner.width.saturating_sub(form_width)) / 2,
                y: inner.y + (inner.height.saturating_sub(form_height)) / 2,
                width: form_width,
                height: form_height,
            };
            frame.render_widget(
                Block::default().style(Style::default().bg(Theme::BASE)),
                form_area,
            );
            self.draw_transfer_form(frame, form_area);
        }

        // Deal form overlay
        if self.dealing {
            let form_height = 5u16;
            let form_width = 50u16.min(inner.width);
            let form_area = Rect {
                x: inner.x + (inner.width.saturating_sub(form_width)) / 2,
                y: inner.y + (inner.height.saturating_sub(form_height)) / 2,
                width: form_width,
                height: form_height,
            };
            frame.render_widget(
                Block::default().style(Style::default().bg(Theme::BASE)),
                form_area,
            );
            self.draw_deal_form(frame, form_area);
        }

        // Add token form overlay
        if self.adding_token {
            let form_height = 5u16;
            let form_width = 55u16.min(inner.width);
            let form_area = Rect {
                x: inner.x + (inner.width.saturating_sub(form_width)) / 2,
                y: inner.y + (inner.height.saturating_sub(form_height)) / 2,
                width: form_width,
                height: form_height,
            };
            frame.render_widget(
                Block::default().style(Style::default().bg(Theme::BASE)),
                form_area,
            );
            self.draw_add_token_form(frame, form_area);
        }

        // Edit fork URL form overlay
        if self.editing_fork_url {
            let form_height = 5u16;
            let form_width = 70u16.min(inner.width);
            let form_area = Rect {
                x: inner.x + (inner.width.saturating_sub(form_width)) / 2,
                y: inner.y + (inner.height.saturating_sub(form_height)) / 2,
                width: form_width,
                height: form_height,
            };
            frame.render_widget(
                Block::default().style(Style::default().bg(Theme::BASE)),
                form_area,
            );
            self.draw_edit_fork_url_form(frame, form_area);
        }
    }
}
