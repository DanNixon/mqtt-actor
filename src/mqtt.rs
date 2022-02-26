use super::{Cli, Event};
use anyhow::Result;
use paho_mqtt::{AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, Message};
use std::env;
use tokio::{sync::broadcast::Sender, task::JoinHandle};

pub(crate) fn run(tx: Sender<Event>, args: &Cli) -> Result<JoinHandle<()>> {
    let client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(&args.mqtt_broker)
            .client_id(&args.mqtt_client_id)
            .persistence(env::temp_dir())
            .finalize(),
    )?;

    client
        .connect(
            ConnectOptionsBuilder::new()
                .user_name(&args.mqtt_username)
                .password(&args.mqtt_password)
                .finalize(),
        )
        .wait()?;

    let mut rx = tx.subscribe();

    Ok(tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Event::Exit => {
                    log::debug! {"MQTT task exit"};
                    return;
                }
                Event::SendMessage(msg) => {
                    match client.try_publish(Message::new(msg.topic, msg.message, 1)) {
                        Ok(delivery_token) => {
                            if let Err(e) = delivery_token.wait() {
                                log::error! {"Error sending message: {}", e};
                            }
                        }
                        Err(e) => log::error! {"Error creating/queuing the message: {}", e},
                    }
                }
                _ => {}
            }
        }
    }))
}
