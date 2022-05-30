use super::{Cli, Event};
use anyhow::Result;
use paho_mqtt::{
    AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder, Message, PersistenceType,
};
use std::time::Duration;
use tokio::{sync::broadcast::Sender, task::JoinHandle};

pub(crate) fn run(tx: Sender<Event>, args: &Cli) -> Result<JoinHandle<()>> {
    let client = AsyncClient::new(
        CreateOptionsBuilder::new()
            .server_uri(&args.mqtt_broker)
            .client_id(&args.mqtt_client_id)
            .persistence(PersistenceType::None)
            .finalize(),
    )?;

    client.set_connected_callback(|_| {
        log::info!("Connected to broker");
    });

    let response = client
        .connect(
            ConnectOptionsBuilder::new()
                .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(5))
                .keep_alive_interval(Duration::from_secs(5))
                .user_name(&args.mqtt_username)
                .password(&args.mqtt_password)
                .finalize(),
        )
        .wait()?;

    log::info!(
        "Using MQTT version {}",
        response.connect_response().unwrap().mqtt_version
    );

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
                        }
                    }
                }
                _ => {}
            }
        }
    }))
}
