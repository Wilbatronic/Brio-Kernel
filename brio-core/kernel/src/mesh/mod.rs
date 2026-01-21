pub mod types;
pub mod remote;
pub mod grpc;
pub mod service;

pub use types::*;
pub use remote::*;
pub use service::*;

use tokio::sync::oneshot;

#[derive(Debug, Clone)]
pub enum Payload {
    Json(String),
    Binary(Vec<u8>),
}

pub struct MeshMessage {
    pub target: String,
    pub method: String,
    pub payload: Payload,
    // The return channel.
    // We start simple: A result containing a Payload or an Error string.
    pub reply_tx: oneshot::Sender<Result<Payload, String>>,
}
