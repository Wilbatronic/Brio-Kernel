use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a kernel node in the cluster
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl NodeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Address where a node acts as a gRPC server
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeAddress(pub String);

impl fmt::Display for NodeAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata about a registered node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: NodeAddress,
    pub capabilities: Vec<String>,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    pub node_id: String,
    pub listen_address: String,
    pub bootstrap_nodes: Vec<String>,
}
