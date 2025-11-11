use crate::{archipelago, config};
use std::sync::atomic::{AtomicIsize, Ordering};

// Disconnected
pub(crate) static CONNECTION_STATUS: AtomicIsize = AtomicIsize::new(0);

#[tokio::main(flavor = "multi_thread", worker_threads = 1)]
pub async fn send_connect_message(url: String) {
    log::debug!("Connecting to Archipelago");
    match archipelago::TX_ARCH.get() {
        None => log::error!("Connect TX doesn't exist"),
        Some(tx) => {
            tx.send(url)
            .await
            .expect("Failed to send data");
        }
    }
}



pub(crate) fn auto_connect() {
    loop {
        if CONNECTION_STATUS.load(Ordering::SeqCst) != 1 {
            log::debug!("Attempting to connect to local client");
            send_connect_message(
                format!(
                    "{}:{}",
                    config::CONFIG.connections.address,
                    config::CONFIG.connections.port
                )
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(
            config::CONFIG.connections.reconnect_interval_seconds as u64,
        ));
    }
}
