use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq)]
enum CastTab {
    Call,
    Send,
    Balance,
}

impl CastTab {
    fn all() -> Vec<Self> {
        vec![Self::Call, Self::Send, Self::Balance]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Call => "Call",
            Self::Send => "Send",
            Self::Balance => "Balance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum InputField {
    To,
    Sig,
    Args,
    Address,
}

pub struct CastPanel {
    active_tab: CastTab,
    active_field: usize,
    // Form fields
    to_input: String,
    sig_input: String,
    args_input: String,
    address_input: String,
    // Results
    result: Option<String>,
    result_is_error: bool,
    // History
    history: Vec<HistoryEntry>,
    editing: bool,
    action_tx: Option<UnboundedSender<Action>>,
}

#[derive(Debug, Clone)]
struct HistoryEntry {
    command: String,
    result: String,
    success: bool,
}

impl CastPanel {
    pub fn new() -> Self {
        Self {
            active_tab: CastTab::Call,
            active_field: 0,
            to_input: String::new(),
            sig_input: String::new(),
            args_input: String::new(),
            address_input: String::new(),
            result: None,
            result_is_error: false,
            history: Vec::new(),
            editing: false,
            action_tx: None,
        }
    }

    fn fields_for_tab(&self) -> Vec<(&str, &str)> {
        match self.active_tab {
            CastTab::Call => vec![
                ("To", &self.to_input),
                ("Sig", &self.sig_input),
                ("Args", &self.args_input),
            ],
            CastTab::Send => vec![
                ("To", &self.to_input),
                ("Sig", &self.sig_input),
                ("Args", &self.args_input),
            ],
            CastTab::Balance => vec![("Address", &self.address_input)],
        }
    }

    fn field_count(&self) -> usize {
        match self.active_tab {
            CastTab::Call | CastTab::Send => 3,
            CastTab::Balance => 1,
        }
    }

    fn active_input_mut(&mut self) -> &mut String {
        match self.active_tab {
            CastTab::Call | CastTab::Send => match self.active_field {
                0 => &mut self.to_input,
                1 => &mut self.sig_input,
                2 => &mut self.args_input,
                _ => &mut self.to_input,
            },
            CastTab::Balance => &mut self.address_input,
        }
    }

    fn execute_command(&self) -> Option<Action> {
        match self.active_tab {
            CastTab::Call => {
                let args: Vec<String> = self
                    .args_input
                    .split_whitespace()
                    .map(String::from)
                    .collect();
                Some(Action::CastCall {
                    to: self.to_input.clone(),
                    sig: self.sig_input.clone(),
                    args,
                })
            }
            CastTab::Send => {
                let args: Vec<String> = self
                    .args_input
                    .split_whitespace()
                    .map(String::from)
                    .collect();
                Some(Action::CastSend {
                    to: self.to_input.clone(),
                    sig: self.sig_input.clone(),
                    args,
                })
            }
            CastTab::Balance => Some(Action::CastBalance(self.address_input.clone())),
        }
    }

    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        let tabs: Vec<Line> = CastTab::all()
            .iter()
            .map(|t| Line::from(t.label()))
            .collect();

        let selected = match self.active_tab {
            CastTab::Call => 0,
            CastTab::Send => 1,
            CastTab::Balance => 2,
        };

        let tabs_widget = Tabs::new(tabs)
            .select(selected)
            .style(Style::default().fg(Theme::OVERLAY0))
            .highlight_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::styled(" │ ", Style::default().fg(Theme::SURFACE1)));

        frame.render_widget(tabs_widget, area);
    }

    fn draw_form(&self, frame: &mut Frame, area: Rect) {
        let fields = self.fields_for_tab();
        let mut lines = Vec::new();

        for (i, (label, value)) in fields.iter().enumerate() {
            let is_active = self.editing && i == self.active_field;
            let label_style = if is_active {
                Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::OVERLAY0)
            };

            let value_style = if is_active {
                Style::default().fg(Theme::TEXT)
            } else {
                Style::default().fg(Theme::SUBTEXT0)
            };

            let cursor = if is_active { "█" } else { "" };
            let indicator = if is_active { "▸ " } else { "  " };

            lines.push(Line::from(vec![
                Span::styled(indicator, Style::default().fg(Theme::BLUE)),
                Span::styled(format!("{:<8} ", label), label_style),
                Span::styled(*value, value_style),
                Span::styled(cursor, Style::default().fg(Theme::BLUE)),
            ]));
            lines.push(Line::from(""));
        }

        if !self.editing {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter to edit, j/k to switch fields, Esc to exit",
                Style::default().fg(Theme::OVERLAY0),
            )));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0))
                    .title(" Input ")
                    .title_style(Style::default().fg(Theme::SUBTEXT0)),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn draw_result(&self, frame: &mut Frame, area: Rect) {
        let lines = if let Some(ref result) = self.result {
            let color = if self.result_is_error {
                Theme::RED
            } else {
                Theme::GREEN
            };
            vec![Line::from(Span::styled(
                result.as_str(),
                Style::default().fg(color),
            ))]
        } else {
            vec![Line::from(Span::styled(
                "No results yet",
                Style::default().fg(Theme::OVERLAY0),
            ))]
        };

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0))
                    .title(" Result ")
                    .title_style(Style::default().fg(Theme::SUBTEXT0)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

impl Component for CastPanel {
    fn init(&mut self, action_tx: UnboundedSender<Action>) {
        self.action_tx = Some(action_tx);
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
        if self.editing {
            // Consume ALL keys in editing mode so they don't leak to nav
            match key.code {
                KeyCode::Esc => {
                    self.editing = false;
                }
                KeyCode::Char('j') => {
                    self.active_field = (self.active_field + 1) % self.field_count();
                }
                KeyCode::Char('k') => {
                    self.active_field = if self.active_field == 0 {
                        self.field_count() - 1
                    } else {
                        self.active_field - 1
                    };
                }
                KeyCode::Enter => {
                    self.editing = false;
                    return self.execute_command();
                }
                KeyCode::Char(c) => {
                    self.active_input_mut().push(c);
                }
                KeyCode::Backspace => {
                    self.active_input_mut().pop();
                }
                _ => {}
            }
            return Some(Action::None); // consumed
        }

        // Not editing — only panel-specific shortcut keys
        None
    }

    fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::Select => {
                if !self.editing {
                    self.editing = true;
                    self.active_field = 0;
                }
            }
            Action::CastResult(result) => {
                let cmd = match self.active_tab {
                    CastTab::Call => format!("cast call {} {}", self.to_input, self.sig_input),
                    CastTab::Send => format!("cast send {} {}", self.to_input, self.sig_input),
                    CastTab::Balance => format!("cast balance {}", self.address_input),
                };
                self.history.push(HistoryEntry {
                    command: cmd,
                    result: result.clone(),
                    success: true,
                });
                self.result = Some(result.clone());
                self.result_is_error = false;
                return Some(Action::SetStatus("Cast command completed".to_string()));
            }
            Action::CastError(err) => {
                self.result = Some(err.clone());
                self.result_is_error = true;
                return Some(Action::Error(err.clone()));
            }
            Action::NextTab => {
                self.active_tab = match self.active_tab {
                    CastTab::Call => CastTab::Send,
                    CastTab::Send => CastTab::Balance,
                    CastTab::Balance => CastTab::Call,
                };
                self.active_field = 0;
                self.result = None;
            }
            Action::PrevTab => {
                if self.active_tab == CastTab::Call {
                    return Some(Action::FocusSidebar);
                }
                self.active_tab = match self.active_tab {
                    CastTab::Call => CastTab::Call,
                    CastTab::Send => CastTab::Call,
                    CastTab::Balance => CastTab::Send,
                };
                self.active_field = 0;
                self.result = None;
            }
            _ => {}
        }
        None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BLUE))
            .title(" Cast ")
            .title_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1),  // tabs
            Constraint::Min(8),    // form
            Constraint::Length(5), // result
        ])
        .split(inner);

        self.draw_tabs(frame, chunks[0]);
        self.draw_form(frame, chunks[1]);
        self.draw_result(frame, chunks[2]);
    }
}
