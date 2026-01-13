use anyhow::{Result, anyhow};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::collections::HashMap;
use tokio::sync::broadcast;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::mesh::{MeshMessage, Payload};

pub struct BrioHostState {
    mesh_router: HashMap<String, Sender<MeshMessage>>,
    db_pool: SqlitePool,
    _ui_broadcaster: broadcast::Sender<String>,
}

impl BrioHostState {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new().connect(db_url).await?;

        let (tx, _) = broadcast::channel(100);

        Ok(Self {
            mesh_router: HashMap::new(),
            db_pool: pool,
            _ui_broadcaster: tx,
        })
    }

    /// Register a component (Agent or Tool) with the mesh router.
    /// This enforces the registration contract.
    pub fn register_component(&mut self, id: String, sender: Sender<MeshMessage>) {
        self.mesh_router.insert(id, sender);
    }

    /// Accessor for the DB Pool (Immutable access only)
    pub fn db(&self) -> &SqlitePool {
        &self.db_pool
    }

    pub async fn mesh_call(&self, target: &str, method: &str, payload: Payload) -> Result<Payload> {
        let sender = self
            .mesh_router
            .get(target)
            .ok_or_else(|| anyhow!("Target component '{}' not found", target))?;

        let (reply_tx, reply_rx) = oneshot::channel();

        let message = MeshMessage {
            target: target.to_string(),
            method: method.to_string(),
            payload,
            reply_tx,
        };

        sender
            .send(message)
            .await
            .map_err(|e| anyhow!("Failed to send message to target '{}': {}", target, e))?;

        let response = reply_rx
            .await
            .map_err(|e| anyhow!("Failed to receive reply from target '{}': {}", target, e))?;

        response.map_err(|e| anyhow!("Target '{}' returned error: {}", target, e))
    }
}
