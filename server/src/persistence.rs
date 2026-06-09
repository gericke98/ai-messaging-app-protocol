//! Persistent storage layer using redb for offline queue, dedup cache,
//! TOFU pin store, and thread message history.
//!
//! All Phase 8 persistence uses a single redb database file with four tables.

use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::sync::Arc;

// Table definitions
// Offline queue: key = destination:uuidv7_hex (string), value = JSON bytes
pub const OFFLINE_QUEUE: TableDefinition<&str, &[u8]> = TableDefinition::new("offline_queue");

// Dedup cache: key = relay_id (UUIDv7 string), value = relay_timestamp (RFC 3339 string)
pub const DEDUP_CACHE: TableDefinition<&str, &str> = TableDefinition::new("dedup_cache");

// TOFU pin store: key = server_id (string), value = pinned fingerprint (string)
pub const TOFU_PINS: TableDefinition<&str, &str> = TableDefinition::new("tofu_pins");

// Thread messages: key = "thread_id:sequence_number_padded" (string), value = JSON bytes
pub const THREAD_MESSAGES: TableDefinition<&str, &[u8]> = TableDefinition::new("thread_messages");

/// Convert any Display-able error into String.
fn s(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Shared persistence layer wrapping a redb Database.
#[derive(Clone)]
pub struct PersistenceLayer {
    db: Arc<Database>,
}

impl PersistenceLayer {
    /// Open or create a redb database at the given path.
    /// Creates all four tables on first run.
    pub fn open(path: &str) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;
        // Ensure all tables exist
        let write_txn = db.begin_write()?;
        write_txn.open_table(OFFLINE_QUEUE)?;
        write_txn.open_table(DEDUP_CACHE)?;
        write_txn.open_table(TOFU_PINS)?;
        write_txn.open_table(THREAD_MESSAGES)?;
        write_txn.commit()?;
        Ok(Self { db: Arc::new(db) })
    }

    #[allow(dead_code)]
    pub fn db(&self) -> &Database {
        &self.db
    }

    // --- Dedup Cache ---

    /// Check if relay_id exists in dedup cache. If not, insert it.
    /// Returns Ok(true) if the relay_id is NEW (not a replay).
    /// Returns Ok(false) if the relay_id was ALREADY SEEN (replay detected).
    pub fn check_and_insert_relay_id(
        &self,
        relay_id: &str,
        timestamp: &str,
    ) -> Result<bool, String> {
        let write_txn = self.db.begin_write().map_err(s)?;
        {
            let mut table = write_txn.open_table(DEDUP_CACHE).map_err(s)?;
            if table.get(relay_id).map_err(s)?.is_some() {
                return Ok(false); // Duplicate
            }
            table.insert(relay_id, timestamp).map_err(s)?;
        }
        write_txn.commit().map_err(s)?;
        Ok(true)
    }

    /// Remove dedup entries older than max_age.
    /// Returns count of removed entries.
    pub fn cleanup_expired_dedup(&self, max_age: chrono::Duration) -> Result<usize, String> {
        let cutoff = chrono::Utc::now() - max_age;
        let write_txn = self.db.begin_write().map_err(s)?;
        let mut removed = 0usize;
        {
            let mut table = write_txn.open_table(DEDUP_CACHE).map_err(s)?;
            let mut to_remove = Vec::new();
            for entry in table.iter().map_err(s)? {
                let entry = entry.map_err(s)?;
                let ts_str = entry.1.value();
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(ts_str) {
                    if ts < cutoff {
                        to_remove.push(entry.0.value().to_string());
                    }
                }
            }
            for key in &to_remove {
                table.remove(key.as_str()).map_err(s)?;
                removed += 1;
            }
        }
        write_txn.commit().map_err(s)?;
        Ok(removed)
    }

    // --- TOFU Pins ---

    /// Get the pinned fingerprint for a server.
    pub fn get_tofu_pin(&self, server_id: &str) -> Result<Option<String>, String> {
        let read_txn = self.db.begin_read().map_err(s)?;
        let table = read_txn.open_table(TOFU_PINS).map_err(s)?;
        match table.get(server_id).map_err(s)? {
            Some(val) => Ok(Some(val.value().to_string())),
            None => Ok(None),
        }
    }

    /// Set or update the pinned fingerprint for a server.
    pub fn set_tofu_pin(&self, server_id: &str, fingerprint: &str) -> Result<(), String> {
        let write_txn = self.db.begin_write().map_err(s)?;
        {
            let mut table = write_txn.open_table(TOFU_PINS).map_err(s)?;
            table.insert(server_id, fingerprint).map_err(s)?;
        }
        write_txn.commit().map_err(s)?;
        Ok(())
    }

    // --- Offline Queue ---

    /// Enqueue a message for a destination. Key format: "destination:uuidv7_hex".
    pub fn enqueue_offline(
        &self,
        destination: &str,
        message: &serde_json::Value,
    ) -> Result<(), String> {
        let key = format!("{}:{}", destination, uuid::Uuid::now_v7());
        let value = serde_json::to_vec(message).map_err(s)?;
        let write_txn = self.db.begin_write().map_err(s)?;
        {
            let mut table = write_txn.open_table(OFFLINE_QUEUE).map_err(s)?;
            table.insert(key.as_str(), value.as_slice()).map_err(s)?;
        }
        write_txn.commit().map_err(s)?;
        Ok(())
    }

    /// Drain up to `limit` messages for a destination (FIFO order).
    /// Removes drained messages from the table.
    pub fn drain_offline(
        &self,
        destination: &str,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, String> {
        let prefix = format!("{}:", destination);
        let write_txn = self.db.begin_write().map_err(s)?;
        let mut messages = Vec::new();
        {
            let mut table = write_txn.open_table(OFFLINE_QUEUE).map_err(s)?;
            let mut keys_to_remove = Vec::new();
            for entry in table.range::<&str>(prefix.as_str()..).map_err(s)? {
                let entry = entry.map_err(s)?;
                let key = entry.0.value().to_string();
                if !key.starts_with(&prefix) {
                    break;
                }
                if messages.len() >= limit {
                    break;
                }
                let value: serde_json::Value =
                    serde_json::from_slice(entry.1.value()).map_err(s)?;
                messages.push(value);
                keys_to_remove.push(key);
            }
            for key in &keys_to_remove {
                table.remove(key.as_str()).map_err(s)?;
            }
        }
        write_txn.commit().map_err(s)?;
        Ok(messages)
    }

    /// Count queued messages for a destination.
    pub fn queue_len(&self, destination: &str) -> Result<usize, String> {
        let prefix = format!("{}:", destination);
        let read_txn = self.db.begin_read().map_err(s)?;
        let table = read_txn.open_table(OFFLINE_QUEUE).map_err(s)?;
        let mut count = 0usize;
        for entry in table.range::<&str>(prefix.as_str()..).map_err(s)? {
            let entry = entry.map_err(s)?;
            if !entry.0.value().starts_with(&prefix) {
                break;
            }
            count += 1;
        }
        Ok(count)
    }

    /// Evict oldest messages for a destination if count exceeds `max_count`.
    /// Returns number of evicted messages.
    pub fn evict_oldest(&self, destination: &str, max_count: usize) -> Result<usize, String> {
        let current = self.queue_len(destination)?;
        if current <= max_count {
            return Ok(0);
        }
        let to_remove = current - max_count;
        let prefix = format!("{}:", destination);
        let write_txn = self.db.begin_write().map_err(s)?;
        let mut removed = 0usize;
        {
            let mut table = write_txn.open_table(OFFLINE_QUEUE).map_err(s)?;
            let mut keys = Vec::new();
            for entry in table.range::<&str>(prefix.as_str()..).map_err(s)? {
                let entry = entry.map_err(s)?;
                if !entry.0.value().starts_with(&prefix) || removed >= to_remove {
                    break;
                }
                keys.push(entry.0.value().to_string());
                removed += 1;
            }
            for key in &keys {
                table.remove(key.as_str()).map_err(s)?;
            }
        }
        write_txn.commit().map_err(s)?;
        Ok(removed)
    }

    /// Remove offline queue entries older than max_age (7-day retention per D-12).
    pub fn cleanup_expired_queue(&self, max_age: chrono::Duration) -> Result<usize, String> {
        let cutoff = chrono::Utc::now() - max_age;
        let write_txn = self.db.begin_write().map_err(s)?;
        let mut removed = 0usize;
        {
            let mut table = write_txn.open_table(OFFLINE_QUEUE).map_err(s)?;
            let mut to_remove = Vec::new();
            for entry in table.iter().map_err(s)? {
                let entry = entry.map_err(s)?;
                let key = entry.0.value();
                // Extract UUIDv7 from key "destination:uuidv7"
                if let Some(uuid_str) = key.split(':').next_back() {
                    if let Ok(uuid) = uuid::Uuid::parse_str(uuid_str) {
                        if let Some(ts) = uuid.get_timestamp() {
                            let (secs, _) = ts.to_unix();
                            let entry_time = chrono::DateTime::from_timestamp(secs as i64, 0)
                                .unwrap_or_default();
                            if entry_time < cutoff {
                                to_remove.push(key.to_string());
                            }
                        }
                    }
                }
            }
            for key in &to_remove {
                table.remove(key.as_str()).map_err(s)?;
                removed += 1;
            }
        }
        write_txn.commit().map_err(s)?;
        Ok(removed)
    }

    // --- Thread Messages ---

    /// Store a message in thread history.
    /// Key format: "thread_id:sequence_padded" (zero-padded to 20 digits for sort order).
    pub fn store_thread_message(
        &self,
        thread_id: &str,
        sequence: u64,
        message: &serde_json::Value,
    ) -> Result<(), String> {
        let key = format!("{}:{:020}", thread_id, sequence);
        let value = serde_json::to_vec(message).map_err(s)?;
        let write_txn = self.db.begin_write().map_err(s)?;
        {
            let mut table = write_txn.open_table(THREAD_MESSAGES).map_err(s)?;
            table.insert(key.as_str(), value.as_slice()).map_err(s)?;
        }
        write_txn.commit().map_err(s)?;
        Ok(())
    }

    /// Retrieve thread messages after a given sequence number, with limit.
    /// Returns (messages, has_more, next_cursor).
    /// Sequences are parsed from keys to correctly handle gaps.
    pub fn get_thread_messages(
        &self,
        thread_id: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<(Vec<serde_json::Value>, bool, Option<u64>), String> {
        let start_key = format!("{}:{:020}", thread_id, after_sequence + 1);
        let prefix = format!("{}:", thread_id);
        let read_txn = self.db.begin_read().map_err(s)?;
        let table = read_txn.open_table(THREAD_MESSAGES).map_err(s)?;

        // Collect (sequence, value) pairs to correctly track cursor across gaps
        let mut entries: Vec<(u64, serde_json::Value)> = Vec::new();

        for entry in table.range::<&str>(start_key.as_str()..).map_err(s)? {
            let entry = entry.map_err(s)?;
            let key = entry.0.value();
            if !key.starts_with(&prefix) {
                break;
            }
            if entries.len() > limit {
                break; // Read one extra to detect has_more
            }
            let value: serde_json::Value = serde_json::from_slice(entry.1.value()).map_err(s)?;
            // Extract sequence from key
            let seq = key
                .strip_prefix(&prefix)
                .and_then(|seq_str| seq_str.parse::<u64>().ok())
                .unwrap_or(0);
            entries.push((seq, value));
        }

        let has_more = entries.len() > limit;
        if has_more {
            entries.truncate(limit);
        }

        let next_cursor = if has_more {
            entries.last().map(|(seq, _)| *seq)
        } else {
            None
        };

        let messages: Vec<serde_json::Value> = entries.into_iter().map(|(_, v)| v).collect();
        Ok((messages, has_more, next_cursor))
    }

    /// Get all destination prefixes that have queued messages.
    pub fn pending_destinations(&self) -> Result<Vec<String>, String> {
        let read_txn = self.db.begin_read().map_err(s)?;
        let table = read_txn.open_table(OFFLINE_QUEUE).map_err(s)?;
        let mut destinations = std::collections::HashSet::new();
        for entry in table.iter().map_err(s)? {
            let entry = entry.map_err(s)?;
            let key = entry.0.value();
            // For offline queue, destination is a server domain (e.g., "server-b.local")
            // Key format: "destination:uuidv7" - rfind for last colon
            if let Some(pos) = key.rfind(':') {
                let dest = &key[..pos];
                destinations.insert(dest.to_string());
            }
        }
        Ok(destinations.into_iter().collect())
    }
}
