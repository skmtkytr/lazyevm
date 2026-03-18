use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::panels::PanelId;
use crate::theme::Theme;

pub struct StatusBar {
    pub message: Option<(String, MessageKind)>,
    pub active_panel: PanelId,
    pub anvil_running: bool,
    pub anvil_transferring: bool,
    pub anvil_dealing: bool,
    pub anvil_adding_token: bool,
    pub anvil_editing_fork_url: bool,
    pub network_name: String,
}

#[derive(Debug, Clone)]
pub enum MessageKind {
    Info,
    Success,
    Error,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            message: None,
            active_panel: PanelId::Wallets,
            anvil_running: false,
            anvil_transferring: false,
            anvil_dealing: false,
            anvil_adding_token: false,
            anvil_editing_fork_url: false,
            network_name: String::new(),
        }
    }

    pub fn set_message(&mut self, msg: String, kind: MessageKind) {
        self.message = Some((msg, kind));
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::horizontal([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

        // Left: keybinding hints
        let hints = self.get_hints();
        let hint_spans: Vec<Span> = hints
            .iter()
            .flat_map(|(key, desc)| {
                vec![
                    Span::styled(
                        format!(" {} ", key),
                        Style::default()
                            .fg(Theme::BASE)
                            .bg(Theme::BLUE)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(" {} ", desc), Style::default().fg(Theme::SUBTEXT0)),
                    Span::raw(" "),
                ]
            })
            .collect();

        let hints_line = Line::from(hint_spans);
        frame.render_widget(
            Paragraph::new(hints_line).style(Style::default().bg(Theme::MANTLE)),
            chunks[0],
        );

        // Right: status message + anvil indicator
        let mut right_spans = Vec::new();

        if !self.network_name.is_empty() {
            right_spans.push(Span::styled(
                format!(" {} ", self.network_name),
                Style::default().fg(Theme::SKY),
            ));
            right_spans.push(Span::styled(" │ ", Style::default().fg(Theme::SURFACE1)));
        }

        if self.anvil_running {
            right_spans.push(Span::styled(
                "● anvil ",
                Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
            ));
        }

        if let Some((ref msg, ref kind)) = self.message {
            let color = match kind {
                MessageKind::Info => Theme::BLUE,
                MessageKind::Success => Theme::GREEN,
                MessageKind::Error => Theme::RED,
            };
            right_spans.push(Span::styled(msg.clone(), Style::default().fg(color)));
        }

        let right_line = Line::from(right_spans);
        frame.render_widget(
            Paragraph::new(right_line)
                .style(Style::default().bg(Theme::MANTLE))
                .alignment(ratatui::layout::Alignment::Right),
            chunks[1],
        );
    }

    fn get_hints(&self) -> Vec<(&str, &str)> {
        // Modal form modes have their own hints
        if self.anvil_transferring {
            return vec![
                ("j/k", "field"),
                ("Enter", "send"),
                ("Esc", "cancel"),
            ];
        }
        if self.anvil_dealing {
            return vec![
                ("Enter", "deal"),
                ("Esc", "cancel"),
            ];
        }
        if self.anvil_adding_token {
            return vec![
                ("Enter", "detect"),
                ("Esc", "cancel"),
            ];
        }
        if self.anvil_editing_fork_url {
            return vec![
                ("Enter", "save"),
                ("Esc", "cancel"),
            ];
        }

        let mut hints = vec![("?", "help"), ("jk", "up/down"), ("hl", "tabs")];

        match self.active_panel {
            PanelId::Wallets => {
                hints.extend([("n", "new"), ("i", "import"), ("r", "refresh")]);
            }
            PanelId::Anvil => {
                hints.extend([("s", "start"), ("S", "stop"), ("f", "fork"), ("F", "fork url"), ("m", "mine"), ("t", "transfer")]);
                hints.extend([("D", "deal"), ("E", "eth deal"), ("B", "detect slot"), ("a", "add token")]);
            }
            PanelId::Forge => {
                hints.extend([("b", "build"), ("t", "test"), ("c", "clear")]);
            }
            PanelId::Cast => {
                hints.extend([("Enter", "edit"), ("j/k", "field"), ("Esc", "exit")]);
            }
            PanelId::Explorer => {
                hints.extend([("r", "refresh"), ("Enter", "details")]);
            }
        }

        hints
    }
}
