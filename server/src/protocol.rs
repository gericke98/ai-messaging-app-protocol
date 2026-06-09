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

pub fn new_request(
    method: &str,
    params: serde_json::Value,
    id: serde_json::Value,
) -> JsonRpcRequest {
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

// --- Helpers ---

/// Generate a new UUIDv7 message ID.
pub fn new_message_id() -> String {
    Uuid::now_v7().to_string()
}

/// Get current time as RFC 3339 with millisecond precision.
pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Current Unix time in seconds.
pub fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Build a JSON-RPC error as a raw `serde_json::Value`.
///
/// Useful for handlers that return `Json<serde_json::Value>` instead of
/// the typed `JsonRpcResponse` (e.g., federation relay handler).
pub fn new_error_value(id: serde_json::Value, code: i32, message: &str) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}
