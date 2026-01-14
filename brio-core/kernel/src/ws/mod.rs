//! WebSocket module for JSON Patch broadcasting.

pub mod broadcaster;
pub mod connection;
pub mod handler;
pub mod types;

pub use broadcaster::Broadcaster;
pub use types::{BroadcastMessage, ClientId, WsError, WsPatch};
