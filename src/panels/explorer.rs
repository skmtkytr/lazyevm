use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Row, Table, TableState, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, BlockInfo, TxDetail, TxInfo};
use crate::components::Component;
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq)]
enum ExplorerView {
    Blocks,
    Transactions(u64),
    TxDetail(String),
}

pub struct ExplorerPanel {
    view: ExplorerView,
    blocks: Vec<BlockInfo>,
    transactions: Vec<TxInfo>,
    tx_detail: Option<TxDetail>,
    block_state: TableState,
    tx_state: ListState,
    selected_block: usize,
    selected_tx: usize,
    loading: bool,
    action_tx: Option<UnboundedSender<Action>>,
}

impl ExplorerPanel {
    pub fn new() -> Self {
        Self {
            view: ExplorerView::Blocks,
            blocks: Vec::new(),
            transactions: Vec::new(),
            tx_detail: None,
            block_state: TableState::default(),
            tx_state: ListState::default(),
            selected_block: 0,
            selected_tx: 0,
            loading: false,
            action_tx: None,
        }
    }

    fn draw_blocks(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["Block", "Hash", "Txs", "Gas Used", "Time"])
            .style(
                Style::default()
                    .fg(Theme::OVERLAY0)
                    .add_modifier(Modifier::BOLD),
            )
            .bottom_margin(1);

        let rows: Vec<Row> = self
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let style = if i == self.selected_block {
                    Style::default().fg(Theme::BLUE)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                let hash_short = if b.hash.len() > 16 {
                    format!("{}...", &b.hash[..16])
                } else {
                    b.hash.clone()
                };

                let time = chrono::DateTime::from_timestamp(b.timestamp as i64, 0)
                    .map(|dt| dt.format("%H:%M:%S").to_string())
                    .unwrap_or_default();

                Row::new(vec![
                    format!("#{}", b.number),
                    hash_short,
                    b.tx_count.to_string(),
                    b.gas_used.clone(),
                    time,
                ])
                .style(style)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(20),
                Constraint::Length(6),
                Constraint::Length(14),
                Constraint::Length(10),
            ],
        )
        .header(header)
        .row_highlight_style(Style::default().bg(Theme::SURFACE0));

        frame.render_stateful_widget(table, area, &mut self.block_state);
    }

    fn draw_transactions(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .transactions
            .iter()
            .enumerate()
            .map(|(i, tx)| {
                let style = if i == self.selected_tx {
                    Style::default().fg(Theme::BLUE)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                let status = if tx.status { "✓" } else { "✗" };
                let status_color = if tx.status { Theme::GREEN } else { Theme::RED };

                let hash_short = if tx.hash.len() > 16 {
                    format!("{}...", &tx.hash[..16])
                } else {
                    tx.hash.clone()
                };

                let from_short = if tx.from.len() > 12 {
                    format!("{}...", &tx.from[..12])
                } else {
                    tx.from.clone()
                };

                let to_short = if tx.to.len() > 12 {
                    format!("{}...", &tx.to[..12])
                } else {
                    tx.to.clone()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", status), Style::default().fg(status_color)),
                    Span::styled(format!("{:<18} ", hash_short), style),
                    Span::styled(format!("{} → {} ", from_short, to_short), Style::default().fg(Theme::SUBTEXT0)),
                    Span::styled(&tx.value, Style::default().fg(Theme::GREEN)),
                ]))
            })
            .collect();

        let block_num = if let ExplorerView::Transactions(num) = self.view {
            format!(" Block #{} Transactions ", num)
        } else {
            " Transactions ".to_string()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0))
                    .title(block_num)
                    .title_style(Style::default().fg(Theme::SUBTEXT0)),
            )
            .highlight_style(Style::default().bg(Theme::SURFACE0));

        frame.render_stateful_widget(list, area, &mut self.tx_state);
    }

    fn draw_tx_detail(&self, frame: &mut Frame, area: Rect) {
        if let Some(ref detail) = self.tx_detail {
            let status = if detail.status { "Success" } else { "Failed" };
            let status_color = if detail.status { Theme::GREEN } else { Theme::RED };

            let lines = vec![
                Line::from(vec![
                    Span::styled("Hash:        ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.hash, Style::default().fg(Theme::MAUVE)),
                ]),
                Line::from(vec![
                    Span::styled("Status:      ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(status, Style::default().fg(status_color)),
                ]),
                Line::from(vec![
                    Span::styled("Block:       ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(
                        format!("#{}", detail.block_number),
                        Style::default().fg(Theme::YELLOW),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("From:        ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.from, Style::default().fg(Theme::TEXT)),
                ]),
                Line::from(vec![
                    Span::styled("To:          ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.to, Style::default().fg(Theme::TEXT)),
                ]),
                Line::from(vec![
                    Span::styled("Value:       ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.value, Style::default().fg(Theme::GREEN)),
                ]),
                Line::from(vec![
                    Span::styled("Gas Used:    ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.gas_used, Style::default().fg(Theme::TEXT)),
                ]),
                Line::from(vec![
                    Span::styled("Gas Price:   ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.gas_price, Style::default().fg(Theme::TEXT)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Input:       ", Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(&detail.input, Style::default().fg(Theme::SUBTEXT0)),
                ]),
            ];

            let paragraph = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(Theme::SURFACE0))
                        .title(" Transaction Detail ")
                        .title_style(Style::default().fg(Theme::SUBTEXT0)),
                )
                .wrap(Wrap { trim: false });

            frame.render_widget(paragraph, area);
        }
    }
}

impl Component for ExplorerPanel {
    fn init(&mut self, action_tx: UnboundedSender<Action>) {
        self.action_tx = Some(action_tx.clone());
        let _ = action_tx.send(Action::RefreshBlocks);
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('r') => Some(Action::RefreshBlocks),
            KeyCode::Esc => {
                // Back-navigate within explorer; if at top level, let default handle it
                match &self.view {
                    ExplorerView::Blocks => None,
                    _ => Some(Action::Back),
                }
            }
            _ => None,
        }
    }

    fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::Select => {
                match &self.view {
                    ExplorerView::Blocks => {
                        if let Some(block) = self.blocks.get(self.selected_block) {
                            return Some(Action::SelectBlock(block.number));
                        }
                    }
                    ExplorerView::Transactions(_) => {
                        if let Some(tx) = self.transactions.get(self.selected_tx) {
                            return Some(Action::SelectTx(tx.hash.clone()));
                        }
                    }
                    ExplorerView::TxDetail(_) => {}
                }
            }
            Action::Back => {
                match &self.view {
                    ExplorerView::TxDetail(_) => {
                        self.view = if let Some(block) = self.blocks.get(self.selected_block) {
                            ExplorerView::Transactions(block.number)
                        } else {
                            ExplorerView::Blocks
                        };
                        self.tx_detail = None;
                    }
                    ExplorerView::Transactions(_) => {
                        self.view = ExplorerView::Blocks;
                        self.transactions.clear();
                    }
                    ExplorerView::Blocks => {}
                }
            }
            Action::PrevTab => {
                return Some(Action::FocusSidebar);
            }
            Action::RefreshBlocks => {
                self.loading = true;
            }
            Action::BlocksLoaded(blocks) => {
                self.blocks = blocks.clone();
                self.loading = false;
                if !self.blocks.is_empty() {
                    self.block_state.select(Some(0));
                    self.selected_block = 0;
                }
            }
            Action::SelectBlock(num) => {
                self.view = ExplorerView::Transactions(*num);
                self.selected_tx = 0;
                self.tx_state.select(Some(0));
            }
            Action::TxsLoaded(txs) => {
                self.transactions = txs.clone();
                if !self.transactions.is_empty() {
                    self.tx_state.select(Some(0));
                }
            }
            Action::SelectTx(hash) => {
                self.view = ExplorerView::TxDetail(hash.clone());
            }
            Action::TxDetailLoaded(detail) => {
                self.tx_detail = Some(detail.clone());
            }
            Action::Up => match self.view {
                ExplorerView::Blocks => {
                    if !self.blocks.is_empty() {
                        self.selected_block = self.selected_block.saturating_sub(1);
                        self.block_state.select(Some(self.selected_block));
                    }
                }
                ExplorerView::Transactions(_) => {
                    if !self.transactions.is_empty() {
                        self.selected_tx = self.selected_tx.saturating_sub(1);
                        self.tx_state.select(Some(self.selected_tx));
                    }
                }
                _ => {}
            },
            Action::Down => match self.view {
                ExplorerView::Blocks => {
                    if !self.blocks.is_empty() {
                        self.selected_block =
                            (self.selected_block + 1).min(self.blocks.len() - 1);
                        self.block_state.select(Some(self.selected_block));
                    }
                }
                ExplorerView::Transactions(_) => {
                    if !self.transactions.is_empty() {
                        self.selected_tx =
                            (self.selected_tx + 1).min(self.transactions.len() - 1);
                        self.tx_state.select(Some(self.selected_tx));
                    }
                }
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BLUE))
            .title(if self.loading {
                " Explorer (loading...) "
            } else {
                " Explorer "
            })
            .title_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.blocks.is_empty() && !self.loading {
            let msg = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "No blocks yet. Start anvil or connect to a node.",
                    Style::default().fg(Theme::OVERLAY0),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press 'r' to refresh",
                    Style::default().fg(Theme::SUBTEXT0),
                )),
            ])
            .alignment(ratatui::layout::Alignment::Center);

            frame.render_widget(msg, inner);
            return;
        }

        match &self.view {
            ExplorerView::Blocks => {
                self.draw_blocks(frame, inner);
            }
            ExplorerView::Transactions(_) => {
                let chunks = Layout::vertical([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(inner);

                self.draw_blocks(frame, chunks[0]);
                self.draw_transactions(frame, chunks[1]);
            }
            ExplorerView::TxDetail(_) => {
                self.draw_tx_detail(frame, inner);
            }
        }
    }
}
