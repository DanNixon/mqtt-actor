use super::Event;
use anyhow::Result;
use notify::{
    self,
    event::{self, EventKind, ModifyKind, RenameMode},
    Error, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::path::Path;
use tokio::sync::broadcast::Sender;

pub(crate) fn run(tx: Sender<Event>, path: &Path) -> Result<RecommendedWatcher> {
    let mut watcher =
        notify::recommended_watcher(move |event: std::result::Result<event::Event, Error>| {
            if let Ok(event) = event {
                if event
                    .paths
                    .iter()
                    .map(|p| match p.extension() {
                        Some(ext) => ext == "txt",
                        None => false,
                    })
                    .any(|m| m)
                    && matches!(
                        event.kind,
                        EventKind::Create(_)
                            | EventKind::Modify(ModifyKind::Data(_))
                            | EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                            | EventKind::Remove(_)
                    )
                {
                    log::debug!(
                        "Got filesystem event that is probably a script file: {:?}",
                        event
                    );
                    if let Err(e) = tx.send(Event::ReloadScript) {
                        log::error!("Failed to send reload trigger: {}", e);
                    }
                }
            }
        })?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    Ok(watcher)
}
