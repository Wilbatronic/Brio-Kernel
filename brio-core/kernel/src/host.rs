use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use tokio::sync::broadcast;
use anyhow::Result;

// Placeholder for MeshMessage - will be defined in mesh mod later
pub struct MeshMessage {
    pub payload: Vec<u8>,
}

pub struct BrioHostState {
    pub mesh_router: HashMap<String, Sender<MeshMessage>>,
    pub db_pool: SqlitePool,
    pub ui_broadcaster: broadcast::Sender<String>,
}

impl BrioHostState {
    pub async fn new(db_url: &str) -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .connect(db_url)
            .await?;
            
        let (tx, _) = broadcast::channel(100);

        Ok(Self {
            mesh_router: HashMap::new(),
            db_pool: pool,
            ui_broadcaster: tx,
        })
    }
}
