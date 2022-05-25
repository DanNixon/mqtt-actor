use super::{Cli, Event};
use anyhow::{anyhow, Result};
use paho_mqtt::{AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, Message};
use std::{env, time::Duration};
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
    let qos = args.mqtt_qos;

    Ok(tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                Event::Exit => {
                    log::debug!("Task exit");
                    return;
                }
                Event::SendMessage(msg) => {
                    match client.try_publish(Message::new(msg.topic, msg.message, qos)) {
                        Ok(delivery_token) => {
                            if let Err(e) = delivery_token.wait() {
                                log::error!("Error sending message: {}", e);
                            }
                        }
                        Err(e) => {
                            log::error!("Error creating/queuing message: {}", e);
                            if let Err(e) = try_reconnect(&client).await {
                                log::error!("Failed to reconnect: {}", e);
                                tx.send(Event::Exit).unwrap();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }))
}

async fn try_reconnect(c: &AsyncClient) -> Result<()> {
    for i in 0..10 {
        log::info!("Attempting reconnection {}...", i);
        match c.reconnect().await {
            Ok(_) => {
                log::info!("Reconnection successful");
                return Ok(());
            }
            Err(e) => {
                log::error!("Reconnection failed: {}", e);
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err(anyhow!("Failed to reconnect to broker"))
}
