use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

pub type Tui = Terminal<CrosstermBackend<std::io::Stderr>>;

pub fn init() -> color_eyre::Result<Tui> {
    enable_raw_mode()?;
    execute!(std::io::stderr(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(std::io::stderr());
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

pub fn restore() -> color_eyre::Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stderr(), LeaveAlternateScreen)?;
    Ok(())
}
