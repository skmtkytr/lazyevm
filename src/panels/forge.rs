use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Frame,
};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::components::Component;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ForgeTab {
    Build,
    Test,
    Script,
}

impl ForgeTab {
    fn all() -> Vec<Self> {
        vec![Self::Build, Self::Test, Self::Script]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Build => "Build",
            Self::Test => "Test",
            Self::Script => "Script",
        }
    }
}

pub struct ForgePanel {
    active_tab: ForgeTab,
    output_lines: Vec<OutputLine>,
    running: bool,
    last_summary: Option<String>,
    scroll: u16,
    script_path: String,
    action_tx: Option<UnboundedSender<Action>>,
}

#[derive(Debug, Clone)]
struct OutputLine {
    text: String,
    kind: OutputKind,
}

#[derive(Debug, Clone)]
enum OutputKind {
    Normal,
    Pass,
    Fail,
    Warning,
    Header,
}

impl ForgePanel {
    pub fn new() -> Self {
        Self {
            active_tab: ForgeTab::Build,
            output_lines: Vec::new(),
            running: false,
            last_summary: None,
            scroll: 0,
            script_path: String::new(),
            action_tx: None,
        }
    }

    fn classify_line(text: &str) -> OutputKind {
        if text.contains("[PASS]") || text.contains("ok") {
            OutputKind::Pass
        } else if text.contains("[FAIL]") || text.contains("FAIL") {
            OutputKind::Fail
        } else if text.contains("Warning") || text.contains("warning") {
            OutputKind::Warning
        } else if text.starts_with("Compiling")
            || text.starts_with("Running")
            || text.starts_with("Compiler")
            || text.starts_with("Suite")
        {
            OutputKind::Header
        } else {
            OutputKind::Normal
        }
    }

    fn draw_tabs(&self, frame: &mut Frame, area: Rect) {
        let tabs: Vec<Line> = ForgeTab::all()
            .iter()
            .map(|t| Line::from(t.label()))
            .collect();

        let selected = match self.active_tab {
            ForgeTab::Build => 0,
            ForgeTab::Test => 1,
            ForgeTab::Script => 2,
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

    fn draw_output(&self, frame: &mut Frame, area: Rect) {
        let lines: Vec<Line> = self
            .output_lines
            .iter()
            .map(|l| {
                let color = match l.kind {
                    OutputKind::Normal => Theme::TEXT,
                    OutputKind::Pass => Theme::GREEN,
                    OutputKind::Fail => Theme::RED,
                    OutputKind::Warning => Theme::YELLOW,
                    OutputKind::Header => Theme::BLUE,
                };
                Line::from(Span::styled(l.text.as_str(), Style::default().fg(color)))
            })
            .collect();

        let title = if self.running {
            " Output (running...) "
        } else {
            " Output "
        };

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(Theme::SURFACE0))
                    .title(title)
                    .title_style(Style::default().fg(Theme::SUBTEXT0)),
            )
            .scroll((self.scroll, 0))
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}

impl Component for ForgePanel {
    fn init(&mut self, action_tx: UnboundedSender<Action>) {
        self.action_tx = Some(action_tx);
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
        match key.code {
            KeyCode::Char('b') => {
                if !self.running {
                    self.active_tab = ForgeTab::Build;
                    Some(Action::ForgeBuild)
                } else {
                    None
                }
            }
            KeyCode::Char('t') => {
                if !self.running {
                    self.active_tab = ForgeTab::Test;
                    Some(Action::ForgeTest)
                } else {
                    None
                }
            }
            KeyCode::Char('c') => Some(Action::ForgeClear),
            _ => None,
        }
    }

    fn update(&mut self, action: &Action) -> Option<Action> {
        match action {
            Action::ForgeBuild | Action::ForgeTest => {
                self.running = true;
                self.output_lines.clear();
                self.scroll = 0;
                self.last_summary = None;
            }
            Action::ForgeOutput(line) => {
                let kind = Self::classify_line(line);
                self.output_lines.push(OutputLine {
                    text: line.clone(),
                    kind,
                });
                // Auto-scroll to bottom
                let area_height = 20u16; // approximate
                if self.output_lines.len() as u16 > area_height {
                    self.scroll = self.output_lines.len() as u16 - area_height;
                }
            }
            Action::ForgeDone { success, summary } => {
                self.running = false;
                self.last_summary = Some(summary.clone());
                let status = if *success { "succeeded" } else { "failed" };
                return Some(Action::SetStatus(format!("Forge {}", status)));
            }
            Action::ForgeClear => {
                self.output_lines.clear();
                self.scroll = 0;
                self.last_summary = None;
            }
            Action::Up => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            Action::Down => {
                self.scroll += 1;
            }
            Action::NextTab => {
                self.active_tab = match self.active_tab {
                    ForgeTab::Build => ForgeTab::Test,
                    ForgeTab::Test => ForgeTab::Script,
                    ForgeTab::Script => ForgeTab::Build,
                };
            }
            Action::PrevTab => {
                if self.active_tab == ForgeTab::Build {
                    return Some(Action::FocusSidebar);
                }
                self.active_tab = match self.active_tab {
                    ForgeTab::Build => ForgeTab::Build,
                    ForgeTab::Test => ForgeTab::Build,
                    ForgeTab::Script => ForgeTab::Test,
                };
            }
            _ => {}
        }
        None
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Theme::BLUE))
            .title(" Forge ")
            .title_style(
                Style::default()
                    .fg(Theme::BLUE)
                    .add_modifier(Modifier::BOLD),
            );

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::vertical([
            Constraint::Length(1), // tabs
            Constraint::Min(1),    // output
        ])
        .split(inner);

        self.draw_tabs(frame, chunks[0]);

        if self.output_lines.is_empty() && !self.running {
            let hint_lines = match self.active_tab {
                ForgeTab::Build => vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press 'b' to run forge build",
                        Style::default().fg(Theme::OVERLAY0),
                    )),
                ],
                ForgeTab::Test => vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press 't' to run forge test",
                        Style::default().fg(Theme::OVERLAY0),
                    )),
                ],
                ForgeTab::Script => vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press 's' to run forge script",
                        Style::default().fg(Theme::OVERLAY0),
                    )),
                ],
            };

            let paragraph =
                Paragraph::new(hint_lines).alignment(ratatui::layout::Alignment::Center);

            frame.render_widget(paragraph, chunks[1]);
        } else {
            self.draw_output(frame, chunks[1]);
        }
    }
}
