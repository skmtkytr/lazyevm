use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEvent, KeyEventKind};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Tick,
    Render,
    Resize(u16, u16),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64, render_rate_ms: u64) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            let mut reader = EventStream::new();
            let mut tick_interval =
                tokio::time::interval(Duration::from_millis(tick_rate_ms));
            let mut render_interval =
                tokio::time::interval(Duration::from_millis(render_rate_ms));

            loop {
                tokio::select! {
                    event = reader.next() => {
                        match event {
                            Some(Ok(CrosstermEvent::Key(key))) => {
                                if key.kind == KeyEventKind::Press {
                                    let _ = tx.send(Event::Key(key));
                                }
                            }
                            Some(Ok(CrosstermEvent::Resize(w, h))) => {
                                let _ = tx.send(Event::Resize(w, h));
                            }
                            _ => {}
                        }
                    }
                    _ = tick_interval.tick() => {
                        let _ = tx.send(Event::Tick);
                    }
                    _ = render_interval.tick() => {
                        let _ = tx.send(Event::Render);
                    }
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| eyre::eyre!("Event channel closed"))
    }
}
