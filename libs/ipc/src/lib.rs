use bincode;
use config::ty::App;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;

pub use log;

#[derive(Serialize, Deserialize, Debug)]
pub enum StreamResponse<T> {
    Data(T),
    EndOfStream,
}

pub fn start_server<Req, Res, F>(app: App, handler: F) -> std::io::Result<()>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
    F: Fn(Req, Sender<Res>) + Send + Sync + Clone + 'static,
{
    let socket_path = PathBuf::from(format!("/tmp/{}.sock", app.to_string()));
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;
    info!("Server started on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                info!("Accepted connection");
                let handler = handler.clone();
                thread::spawn(move || {
                    let mut len_buf = [0u8; 4];
                    if stream.read_exact(&mut len_buf).is_err() {
                        error!("Failed to read request length");
                        return;
                    }
                    let req_len = u32::from_le_bytes(len_buf) as usize;

                    let mut buf = vec![0u8; req_len];
                    if stream.read_exact(&mut buf).is_err() {
                        error!("Failed to read request data");
                        return;
                    }

                    if let Ok(req) = bincode::deserialize::<Req>(&buf) {
                        info!("Received request: {:?}", req);
                        let (tx, rx) = mpsc::channel();
                        thread::spawn(move || handler(req, tx));

                        for response in rx {
                            trace!("Sending response: {:?}", response);
                            match bincode::serialize(&StreamResponse::Data(response)) {
                                Ok(resp_buf) => {
                                    let len_bytes = (resp_buf.len() as u32).to_le_bytes();
                                    if stream.write_all(&len_bytes).is_err() {
                                        error!("Failed to send response length");
                                        break;
                                    }
                                    if stream.write_all(&resp_buf).is_err() {
                                        error!("Failed to send response data");
                                        break;
                                    }
                                    if stream.flush().is_err() {
                                        error!("Failed to flush stream");
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize response: {}", e);
                                    break;
                                }
                            }
                        }

                        // Send EndOfStream message
                        match bincode::serialize(&StreamResponse::<Res>::EndOfStream) {
                            Ok(end_buf) => {
                                let len_bytes = (end_buf.len() as u32).to_le_bytes();
                                if stream.write_all(&len_bytes).is_err() {
                                    error!("Failed to send EndOfStream length");
                                }
                                if stream.write_all(&end_buf).is_err() {
                                    error!("Failed to send EndOfStream data");
                                }
                                let _ = stream.flush();
                                info!("Stream ended successfully");
                            }
                            Err(e) => error!("Failed to serialize EndOfStream: {}", e),
                        }
                    } else {
                        error!("Failed to deserialize request");
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                continue;
            }
        }
    }
    Ok(())
}

pub fn send_command<App, Req, Res, H>(
    app: App,
    command: &Req,
    handler: Option<H>,
) -> std::io::Result<()>
where
    App: ToString,
    Req: Serialize,
    Res: for<'de> Deserialize<'de> + std::fmt::Debug, // Debug logging
    H: Fn(Res) + Send + 'static,
{
    let socket_path = PathBuf::from(format!("/tmp/{}.sock", app.to_string()));
    info!("Connecting to server at {:?}", socket_path);
    let mut stream = UnixStream::connect(&socket_path)?;

    // Send request with length prefix
    let data = bincode::serialize(command).expect("Serialization failed");
    let len_bytes = (data.len() as u32).to_le_bytes();
    stream.write_all(&len_bytes)?;
    stream.write_all(&data)?;
    stream.flush()?;
    info!("Command sent");

    let mut reader = BufReader::new(stream);

    loop {
        // Read the length of the incoming response
        let mut len_buf = [0u8; 4];
        if let Err(_) = reader.read_exact(&mut len_buf) {
            error!("Failed to read response length");
            break;
        }
        let response_len = u32::from_le_bytes(len_buf) as usize;

        // Read the full response
        let mut buf = vec![0u8; response_len];
        if let Err(_) = reader.read_exact(&mut buf) {
            error!("Failed to read response data");
            break;
        }

        // Deserialize response
        match bincode::deserialize::<StreamResponse<Res>>(&buf) {
            Ok(StreamResponse::Data(response)) => {
                info!("Received response: {:?}", response);
                if let Some(ref handler) = handler {
                    handler(response);
                }
            }
            Ok(StreamResponse::EndOfStream) => {
                info!("End of stream received");
                break;
            }
            Err(e) => {
                error!("Failed to deserialize response: {}", e);
                break;
            }
        }
    }
    Ok(())
}
