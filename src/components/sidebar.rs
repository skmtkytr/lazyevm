use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
};

use crate::panels::PanelId;
use crate::theme::Theme;

pub struct Sidebar {
    pub panels: Vec<PanelId>,
    pub active: usize,
    pub state: ListState,
    pub focused: bool,
}

impl Sidebar {
    pub fn new() -> Self {
        let panels = vec![
            PanelId::Wallets,
            PanelId::Anvil,
            PanelId::Forge,
            PanelId::Cast,
            PanelId::Explorer,
        ];
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            panels,
            active: 0,
            state,
            focused: false,
        }
    }

    pub fn select(&mut self, panel: PanelId) {
        if let Some(idx) = self.panels.iter().position(|p| *p == panel) {
            self.active = idx;
            self.state.select(Some(idx));
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let border_color = if self.focused {
            Theme::BLUE
        } else {
            Theme::SURFACE1
        };

        let items: Vec<ListItem> = self
            .panels
            .iter()
            .enumerate()
            .map(|(i, panel)| {
                let number = format!("{}", i + 1);
                let name = panel.label();
                let icon = panel.icon();

                let style = if i == self.active {
                    Style::default()
                        .fg(Theme::BLUE)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::SUBTEXT0)
                };

                let line = Line::from(vec![
                    Span::styled(format!("{} ", number), Style::default().fg(Theme::OVERLAY0)),
                    Span::styled(format!("{} ", icon), style),
                    Span::styled(name, style),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .title(" lazyevm ")
                    .title_style(Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
            )
            .highlight_style(
                Style::default()
                    .bg(Theme::SURFACE0)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_stateful_widget(list, area, &mut self.state);
    }
}
