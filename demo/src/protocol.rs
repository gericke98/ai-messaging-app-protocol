use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// --- JSON-RPC 2.0 Types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// --- Constructors ---

pub fn new_request(method: &str, params: serde_json::Value, id: serde_json::Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        id,
        params: Some(params),
    }
}

pub fn new_response(id: serde_json::Value, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    }
}

pub fn new_error(id: serde_json::Value, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
            data: None,
        }),
    }
}

pub fn new_notification(method: &str, params: serde_json::Value) -> JsonRpcNotification {
    JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params: Some(params),
    }
}

// --- AINM Message Envelope ---

/// AINM message envelope per security.md Section 3.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub id: String,
    pub from: String,
    pub thread_id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub content: Vec<serde_json::Value>,
    #[serde(default)]
    pub signature: String,
}

impl MessageEnvelope {
    /// Convert to a serde_json::Value for signing and transport.
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "from": self.from,
            "thread_id": self.thread_id,
            "timestamp": self.timestamp,
            "type": self.type_,
            "content": self.content,
            "signature": self.signature,
        })
    }

    /// Parse from a serde_json::Value.
    pub fn from_value(v: &serde_json::Value) -> Option<Self> {
        Some(MessageEnvelope {
            id: v.get("id")?.as_str()?.to_string(),
            from: v.get("from")?.as_str()?.to_string(),
            thread_id: v.get("thread_id")?.as_str()?.to_string(),
            timestamp: v.get("timestamp")?.as_str()?.to_string(),
            type_: v.get("type")?.as_str()?.to_string(),
            content: v.get("content")?.as_array()?.clone(),
            signature: v
                .get("signature")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }
}

// --- Helpers ---

/// Generate a new UUIDv7 message ID.
pub fn new_message_id() -> String {
    Uuid::now_v7().to_string()
}

/// Get current time as RFC 3339 with millisecond precision.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}
