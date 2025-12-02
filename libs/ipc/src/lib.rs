use bincode;
use log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read, Write};
use std::marker::PhantomData;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};

pub use log;

#[derive(Serialize, Deserialize, Debug)]
pub enum StreamResponse<T> {
    Data(T),
    EndOfStream,
}

pub(crate) fn handle<Res, Req>(stream: &mut UnixStream) -> Option<(Req, Sender<Res>)>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    let mut len_buf = [0u8; 4];
    if stream.read_exact(&mut len_buf).is_err() {
        error!("Failed to read request length");
        return None;
    }
    let req_len = u32::from_le_bytes(len_buf) as usize;

    let mut buf = vec![0u8; req_len];
    if stream.read_exact(&mut buf).is_err() {
        error!("Failed to read request data");
        return None;
    }

    if let Ok(req) = bincode::deserialize::<Req>(&buf) {
        info!("Received request: {:?}", req);
        let (tx, rx) = mpsc::channel();

        std::thread::scope(|_| {
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
        });

        Some((req, tx))
    } else {
        error!("Failed to deserialize request");
        None
    }
}

/// Spawns a server that listens for requests, then
/// spawns new (std) threads to handle them.
pub fn start_server<Req, Res, F>(socket_name: impl ToString, handler: F) -> std::io::Result<()>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
    F: Fn(Req, Sender<Res>) + Send + Sync + Clone + 'static,
{
    let socket_path = PathBuf::from(format!("/tmp/{}.sock", socket_name.to_string()));
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;
        let mut perms = std::fs::metadata(&socket_path)?.permissions();
        perms.set_mode(0o777);
        if let Err(e) = std::fs::set_permissions(&socket_path, perms) {
            warn!("Unable to upen up permissions for socket: {e:?}")
        };

    info!("Server started on {:?}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                info!("Accepted connection");
                let (req, tx) = handle(&mut stream).ok_or(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Handshake error",
                ))?;
                (handler.clone())(req, tx);
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

        Ok(Self {
            unix_stream: listener,
            _req: PhantomData,
            _res: PhantomData,
        })
    }
}

impl<Req, Res> futures::Stream for RequestStream<Req, Res>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    type Item = std::io::Result<(Req, Sender<Res>)>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.unix_stream.poll_accept(cx) {
            std::task::Poll::Ready(v) => std::task::Poll::Ready(
                v.ok()
                    .map(|(stream, _)| {
                        Some(
                            match handle(
                                &mut stream
                                    .into_std()
                                    .expect("Failed to construct stdio UnixStream"),
                            ) {
                                Some((req, tx)) => Ok((req, tx)),
                                None => Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "Handshake error",
                                )),
                            },
                        )
                    })
                    .flatten(),
            ),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

/// Spawns a server that delivers requests as a stream.
/// Good for use in select!{} or alike.
pub async fn start_stream<Req, Res, F>(
    socket_name: impl ToString,
) -> std::io::Result<RequestStream<Req, Res>>
where
    Req: for<'de> Deserialize<'de> + Send + 'static + std::fmt::Debug,
    Res: Serialize + Send + 'static + std::fmt::Debug,
{
    RequestStream::new(socket_name).await
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
