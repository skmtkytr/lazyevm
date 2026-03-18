pub mod sidebar;
pub mod status_bar;

use crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use ratatui::Frame;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;

pub trait Component {
    /// Initialize the component with the action sender for async operations
    fn init(&mut self, _action_tx: UnboundedSender<Action>) {}

    /// Handle key events, return an optional action
    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
        let _ = key;
        None
    }

    /// Update state based on an action, return an optional follow-up action
    fn update(&mut self, action: &Action) -> Option<Action> {
        let _ = action;
        None
    }

    /// Render the component
    fn draw(&mut self, frame: &mut Frame, area: Rect);
}
