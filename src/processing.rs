use super::{script::Script, Cli, Event};
use anyhow::Result;
use tokio::{sync::broadcast::Sender, task::JoinHandle};

pub(crate) fn run(tx: Sender<Event>, args: &Cli) -> Result<JoinHandle<()>> {
    let mut rx = tx.subscribe();

    let mut script = Script::new(&args.script_source_dir, args.script_delimiter)?;

    Ok(tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Event::Exit => {
                    log::debug!("Task exit");
                    return;
                }
                Event::ReloadScript => {
                    if let Err(e) = script.reload() {
                        log::error!("Failed to reload script: {}", e);
                    }
                }
                Event::Tick => {
                    for message in script.poll() {
                        log::info!("Sending message: {:?}", message);
                        if let Err(e) = tx.send(Event::SendMessage(message)) {
                            log::error!("Failed to send send message event: {}", e);
                        }
                    }
                }
                _ => {}
            }
        }
    }))
}
