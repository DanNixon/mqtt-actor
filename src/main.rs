mod file_watch;
mod mqtt;
mod processing;
mod script;
mod tick;

use anyhow::{anyhow, Result};
use clap::Parser;
use env_logger::Env;
use script::Message;
use std::path::PathBuf;
use tokio::{signal, sync::broadcast};

/// A simple tool to schedule MQTT messages
#[derive(Debug, Parser)]
struct Cli {
    /// Address of MQTT broker to connect to
    #[clap(long, env = "MQTT_BROKER", default_value = "tcp://localhost:1883")]
    mqtt_broker: String,

    /// Client ID to use when connecting to MQTT broker
    #[clap(long, env = "MQTT_CLIENT_ID", default_value = "mqtt-actor")]
    mqtt_client_id: String,

    /// MQTT username
    #[clap(long, env = "MQTT_USERNAME", default_value = "")]
    mqtt_username: String,

    /// MQTT password
    #[clap(long, env = "MQTT_PASSWORD", default_value = "")]
    mqtt_password: String,

    /// Script file delimiter
    #[clap(long, env = "SCRIPT_DELIMITER", default_value_t = b'|')]
    script_delimiter: u8,

    /// Directory to watch for script files
    script_source_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Event {
    Tick,
    ReloadScript,
    SendMessage(Message),
    Exit,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args = Cli::parse();
    log::debug! {"{:?}", args};

    if !args.script_source_dir.is_dir() {
        return Err(
            anyhow! {"Path \"{}\" is not an accessible directory", args.script_source_dir.display()},
        );
    }

    let (tx, mut rx) = broadcast::channel::<Event>(16);

    let _file_watcher = file_watch::run(tx.clone(), &args.script_source_dir)?;

    let tasks = vec![
        tick::run(tx.clone()),
        mqtt::run(tx.clone(), &args)?,
        processing::run(tx.clone(), &args)?,
    ];

    loop {
        let should_exit = tokio::select! {
            _ = signal::ctrl_c() => true,
            event = rx.recv() => matches!(event, Ok(Event::Exit)),
        };
        if should_exit {
            break;
        }
    }

    log::info! {"Terminating..."};
    tx.send(Event::Exit)?;
    for handle in tasks {
        if let Err(e) = handle.await {
            log::error! {"Failed waiting for task to finish: {}", e};
        }
    }

    Ok(())
}
