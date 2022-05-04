use super::Event;
use std::time::Duration;
use tokio::{sync::broadcast::Sender, task::JoinHandle};

pub(crate) fn run(tx: Sender<Event>) -> JoinHandle<()> {
    let mut rx = tx.subscribe();

    tokio::spawn(async move {
        loop {
            while let Ok(event) = rx.try_recv() {
                if event == Event::Exit {
                    log::debug!("Task exit");
                    return;
                }
            }
            if let Err(e) = tx.send(Event::Tick) {
                log::error!("Failed to send tick event: {}", e);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}
