//! Broadcaster service for JSON Patch distribution.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::broadcast;
use tracing::{debug, warn};

use crate::ws::types::{BroadcastMessage, WsError};

const BROADCAST_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct Broadcaster {
    sender: broadcast::Sender<BroadcastMessage>,
    client_count: Arc<AtomicUsize>,
}

impl Broadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            sender,
            client_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn subscribe(&self) -> BroadcastReceiver {
        self.client_count.fetch_add(1, Ordering::SeqCst);
        debug!(client_count = self.client_count(), "Client subscribed");
        BroadcastReceiver {
            inner: self.sender.subscribe(),
            client_count: Arc::clone(&self.client_count),
        }
    }

    pub fn broadcast(&self, message: BroadcastMessage) -> Result<(), WsError> {
        match self.sender.send(message) {
            Ok(receiver_count) => {
                debug!(receiver_count, "Broadcast sent");
                Ok(())
            }
            Err(_) => {
                warn!("Broadcast sent but no clients connected");
                Ok(())
            }
        }
    }

    pub fn client_count(&self) -> usize {
        self.client_count.load(Ordering::SeqCst)
    }

    pub fn sender(&self) -> &broadcast::Sender<BroadcastMessage> {
        &self.sender
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

pub struct BroadcastReceiver {
    inner: broadcast::Receiver<BroadcastMessage>,
    client_count: Arc<AtomicUsize>,
}

impl BroadcastReceiver {
    pub async fn recv(&mut self) -> Result<BroadcastMessage, WsError> {
        self.inner.recv().await.map_err(|e| match e {
            broadcast::error::RecvError::Closed => WsError::ChannelClosed,
            broadcast::error::RecvError::Lagged(count) => {
                warn!(skipped = count, "Receiver lagged");
                WsError::ChannelClosed
            }
        })
    }
}

impl Drop for BroadcastReceiver {
    fn drop(&mut self) {
        self.client_count.fetch_sub(1, Ordering::SeqCst);
        debug!(
            client_count = self.client_count.load(Ordering::SeqCst),
            "Client unsubscribed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn broadcaster_tracks_client_count() {
        let broadcaster = Broadcaster::new();
        assert_eq!(broadcaster.client_count(), 0);

        let _rx1 = broadcaster.subscribe();
        assert_eq!(broadcaster.client_count(), 1);

        let _rx2 = broadcaster.subscribe();
        assert_eq!(broadcaster.client_count(), 2);

        drop(_rx1);
        assert_eq!(broadcaster.client_count(), 1);
    }

    #[tokio::test]
    async fn broadcast_reaches_subscribers() {
        let broadcaster = Broadcaster::new();
        let mut rx = broadcaster.subscribe();

        broadcaster.broadcast(BroadcastMessage::Shutdown).unwrap();

        let msg = rx.recv().await.unwrap();
        assert!(matches!(msg, BroadcastMessage::Shutdown));
    }

    #[tokio::test]
    async fn broadcast_with_no_subscribers_succeeds() {
        let broadcaster = Broadcaster::new();
        let result = broadcaster.broadcast(BroadcastMessage::Shutdown);
        assert!(result.is_ok());
    }

    #[test]
    fn broadcaster_is_clone() {
        let broadcaster = Broadcaster::new();
        let cloned = broadcaster.clone();
        assert_eq!(broadcaster.client_count(), cloned.client_count());
    }
}
