#[cfg(feature = "bincode")]
use bincode;

#[cfg(not(feature = "bincode"))]
use serde_json;

use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::io::{BufReader, Read, Write};
use std::marker::PhantomData;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};

pub use log;

const MAX_FRAME_SIZE: usize = 8 * 1024 * 1024;

fn serialize_to_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    #[cfg(feature = "bincode")]
    {
        bincode::serialize(value).map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "bincode"))]
    {
        serde_json::to_vec(value).map_err(|e| e.to_string())
    }
}

fn deserialize_from_slice<'de, T>(bytes: &'de [u8]) -> Result<T, String>
where
    T: Deserialize<'de>,
{
    #[cfg(feature = "bincode")]
    {
        bincode::deserialize(bytes).map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "bincode"))]
    {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum StreamResponse<T> {
    Data(T),
    EndOfStream,
}

pub(crate) fn process<Res, Req>(mut stream: UnixStream) -> Option<(Req, Sender<Res>)>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).is_err() {
        error!("Failed to read request length");
        return None;
    }

    trace!("Length bytes: {:?}", len_buf);
    let req_len = u32::from_le_bytes(len_buf) as usize;

    if req_len > MAX_FRAME_SIZE {
        error!("Request too large: {} bytes", req_len);
        return None;
    }

    let mut buf = vec![0u8; req_len];
    if stream.read_exact(&mut buf).is_err() {
        error!("Failed to read request data");
        return None;
    }

    if let Ok(req) = deserialize_from_slice::<Req>(&buf) {
        info!("Received request: {:?}", req);
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            for response in rx {
                trace!("Sending response: {:?}", response);
                match serialize_to_vec(&StreamResponse::Data(response)) {
                    Ok(resp_buf) => {
                        if resp_buf.len() > MAX_FRAME_SIZE {
                            error!("Response too large: {} bytes", resp_buf.len());
                            break;
                        }
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
            match serialize_to_vec(&StreamResponse::<Res>::EndOfStream) {
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
        });

        Some((req, tx))
    } else {
        error!("Failed to deserialize request");
        None
    }
}

/// Spawns a server that listens for requests, then
/// spawns new (std) threads to handle them.
pub fn start_server<Req, Res, F>(socket_path: impl Into<PathBuf> + Display, handler: F) -> std::io::Result<()>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
    F: Fn(Req, Sender<Res>) + Send + Sync + Clone + 'static,
{
    let socket_path = if socket_path.to_string().starts_with("/") {
        socket_path.into()
    } else {
        PathBuf::from(format!("/tmp/{}.sock", socket_path))
    };

    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;

    info!("Server started on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                info!("Accepted connection");
                let handler = handler.clone();
                std::thread::spawn(move || {
                    if let Some((req, tx)) = process(stream) {
                        handler(req, tx);
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

pub struct RequestStream<Req, Res> {
    unix_stream: tokio::net::UnixListener,
    _req: PhantomData<Req>,
    _res: PhantomData<Res>,
}

impl<Req, Res> RequestStream<Req, Res>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    pub async fn new(app: impl ToString) -> std::io::Result<Self> {
        let socket_path = PathBuf::from(format!("/tmp/{}.sock", app.to_string()));
        let _ = std::fs::remove_file(&socket_path);
        let listener = tokio::net::UnixListener::bind(&socket_path)?;
        info!("Server started on {:?}", socket_path);

        Ok(Self { unix_stream: listener, _req: PhantomData, _res: PhantomData })
    }
}

impl<Req, Res> futures::Stream for RequestStream<Req, Res>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    type Item = std::io::Result<(Req, Sender<Res>)>;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
        match self.unix_stream.poll_accept(cx) {
            std::task::Poll::Ready(Ok((stream, _))) => {
                let std_stream = match stream.into_std() {
                    Ok(v) => v,
                    Err(e) => return std::task::Poll::Ready(Some(Err(e))),
                };

                std::task::Poll::Ready(Some(match process(std_stream) {
                    Some((req, tx)) => Ok((req, tx)),
                    None => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Handshake error")),
                }))
            }
            std::task::Poll::Ready(Err(e)) => std::task::Poll::Ready(Some(Err(e))),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// Spawns a server that delivers requests as a stream.
/// Good for use in select!{} or alike.
pub async fn start_stream<Req, Res>(socket_name: impl ToString) -> std::io::Result<RequestStream<Req, Res>>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    RequestStream::new(socket_name).await
}

pub fn send_command<Req, Res, H>(
    socket_path: impl Into<PathBuf> + Display,
    command: &Req,
    handler: Option<H>,
) -> std::io::Result<()>
where
    Req: Serialize,
    Res: for<'de> Deserialize<'de> + std::fmt::Debug, // Debug logging
    H: Fn(Res) + Send + 'static,
{
    let socket_path = if socket_path.to_string().starts_with("/") {
        socket_path.into()
    } else {
        PathBuf::from(format!("/tmp/{}.sock", socket_path))
    };

    info!("Connecting to server at {:?}", socket_path);
    let mut stream = UnixStream::connect(&socket_path)?;

    // Send request with length prefix
    let data = serialize_to_vec(command).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if data.len() > MAX_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Request too large: {} bytes", data.len()),
        ));
    }
    let len_bytes = (data.len() as u32).to_le_bytes();
    stream.write_all(&len_bytes)?;
    stream.write_all(&data)?;
    stream.flush()?;
    info!("Command sent");

    let mut reader = BufReader::new(stream);

    loop {
        // Read the length of the incoming response
        let mut len_buf = [0u8; 4];
        if reader.read_exact(&mut len_buf).is_err() {
            error!("Failed to read response length");
            break;
        }
        let response_len = u32::from_le_bytes(len_buf) as usize;
        if response_len > MAX_FRAME_SIZE {
            error!("Response too large: {} bytes", response_len);
            break;
        }

        // Read the full response
        let mut buf = vec![0u8; response_len];
        if reader.read_exact(&mut buf).is_err() {
            error!("Failed to read response data");
            break;
        }

        // Deserialize response
        match deserialize_from_slice::<StreamResponse<Res>>(&buf) {
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
