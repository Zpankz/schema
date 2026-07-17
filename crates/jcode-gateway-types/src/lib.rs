//! Device-pairing types for the jcode WebSocket gateway.
//!
//! This crate defines the serde-serializable records used by jcode's gateway
//! when remote clients (such as the iOS companion app) pair with a running
//! jcode server: `PairedDevice`, the persisted record of an approved device
//! (id, name, token hash, optional APNs push token, and activity timestamps),
//! and `PairingCode`, the short-lived code a client redeems to complete
//! pairing.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub token_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apns_token: Option<String>,
    pub paired_at: String,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingCode {
    pub code: String,
    pub created_at: String,
    pub expires_at: String,
}
