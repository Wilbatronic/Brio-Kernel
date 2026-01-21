use anyhow::{Result, anyhow};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::inference::{LLMProvider, ProviderRegistry};
use crate::mesh::{MeshMessage, Payload};
use crate::store::{PrefixPolicy, SqlStore};
use crate::vfs::manager::SessionManager;
use crate::ws::{BroadcastMessage, Broadcaster, WsPatch};

pub struct BrioHostState {
    mesh_router: std::sync::RwLock<HashMap<String, Sender<MeshMessage>>>,
    db_pool: SqlitePool,
    broadcaster: Broadcaster,
    session_manager: std::sync::Mutex<SessionManager>,
    provider_registry: Arc<ProviderRegistry>,
}

impl BrioHostState {
    /// Creates a new BrioHostState with a pre-configured provider registry.
    pub async fn new(db_url: &str, registry: ProviderRegistry) -> Result<Self> {
        let pool = SqlitePoolOptions::new().connect(db_url).await?;

        Ok(Self {
            mesh_router: std::sync::RwLock::new(HashMap::new()),
            db_pool: pool,
            broadcaster: Broadcaster::new(),
            session_manager: std::sync::Mutex::new(SessionManager::new()),
            provider_registry: Arc::new(registry),
        })
    }

    /// Creates a new BrioHostState with a single provider (backward compatible).
    pub async fn with_provider(db_url: &str, provider: Box<dyn LLMProvider>) -> Result<Self> {
        let registry = ProviderRegistry::new();
        registry.register_arc("default", Arc::from(provider));
        registry.set_default("default");
        Self::new(db_url, registry).await
    }

    pub fn register_component(&self, id: String, sender: Sender<MeshMessage>) {
        let mut router = self.mesh_router.write().expect("RwLock poisoned");
        router.insert(id, sender);
    }

    pub fn db(&self) -> &SqlitePool {
        &self.db_pool
    }

    pub fn get_store(&self, _scope: &str) -> SqlStore {
        SqlStore::new(self.db_pool.clone(), Box::new(PrefixPolicy))
    }

    pub fn broadcaster(&self) -> &Broadcaster {
        &self.broadcaster
    }

    pub fn broadcast_patch(&self, patch: WsPatch) -> Result<()> {
        self.broadcaster
            .broadcast(BroadcastMessage::Patch(patch))
            .map_err(|e| anyhow!("Broadcast failed: {}", e))
    }

    pub async fn mesh_call(&self, target: &str, method: &str, payload: Payload) -> Result<Payload> {
        let sender = {
            let router = self.mesh_router.read().expect("RwLock poisoned");
            router
                .get(target)
                .ok_or_else(|| anyhow!("Target component '{}' not found", target))?
                .clone()
        };

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

    pub fn begin_session(&self, base_path: String) -> Result<String, String> {
        let mut manager = self.session_manager.lock().expect("Mutex poisoned");
        manager.begin_session(base_path)
    }

    pub fn commit_session(&self, session_id: String) -> Result<(), String> {
        let mut manager = self.session_manager.lock().expect("Mutex poisoned");
        manager.commit_session(session_id)
    }

    /// Returns the provider registry for multi-model access.
    pub fn registry(&self) -> Arc<ProviderRegistry> {
        self.provider_registry.clone()
    }

    /// Returns a specific LLM provider by name.
    pub fn inference_by_name(&self, name: &str) -> Option<Arc<dyn LLMProvider>> {
        self.provider_registry.get(name)
    }

    /// Returns the default LLM provider (backward compatible).
    pub fn inference(&self) -> Option<Arc<dyn LLMProvider>> {
        self.provider_registry.get_default()
    }
}

