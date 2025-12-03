use core::panic;
use std::{process, time::Duration};

use tokio::{select, spawn, time::sleep};

use ipsea::{send_command, start_server};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let server = spawn(async {
        start_server(
            "ipsea-test",
            |i: String, o: std::sync::mpsc::Sender<String>| {
                o.send(i).expect("Failed to eecho message");
            },
        )
        .expect("Failed to start server")
    });

    sleep(Duration::from_secs(1)).await;

    let message: String = "Hello, world".into();
    let (tx, mut all_good) = tokio::sync::mpsc::channel::<()>(1);

    let client = spawn(async move {
        send_command(
            "ipsea-test",
            &message,
            Some({
                let message = message.clone();
                let tx = tx.clone();
                move |res: String| {
                    assert_eq!(res, message);
                    server.abort();
                    let _ = tx.send(());
                }
            }),
        )
        .expect("Failed to send command");
    });

    select! {
        _ = all_good.recv() => {
            client.abort();
        },
        _ = sleep(Duration::from_secs(1)) => {
            eprintln!("Server took too long");
            process::exit(1);
        }
    }
}
