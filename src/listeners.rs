//! Shared listener management for desktop platforms.
//!
//! This is a replication of Tauri's plugin listener implementation which is
//! currently only available for mobile plugins. Once Tauri adds desktop support
//! for plugin listeners, this module can be removed.
//!
//! Provides channel-based event delivery for transaction updates and other IAP events.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use crate::error::{ErrorResponse, PluginInvokeError};

type ChannelMap = HashMap<u32, tauri::ipc::Channel<serde_json::Value>>;
type ListenerMap = HashMap<String, ChannelMap>;

static LISTENERS: OnceLock<RwLock<ListenerMap>> = OnceLock::new();

/// Initialize the listeners registry. Call this during plugin init.
pub fn init() {
    let _ = LISTENERS.get_or_init(|| RwLock::new(HashMap::new()));
}

/// Trigger an event to all registered listeners for the given event name.
///
/// Called by platform-specific code when transaction updates occur.
#[allow(dead_code)]
pub fn trigger(event: &str, payload: &str) -> crate::Result<()> {
    let listeners = LISTENERS.get().ok_or_else(|| {
        crate::Error::from(PluginInvokeError::InvokeRejected(ErrorResponse {
            code: None,
            message: Some("Listeners not initialized".to_string()),
            data: (),
        }))
    })?;

    // Clone the channel set out of the guard, then drop the lock before
    // parsing/sending to avoid holding a read lock across slow operations.
    let channels = {
        let guard = listeners.read().map_err(|e| {
            crate::Error::from(PluginInvokeError::InvokeRejected(ErrorResponse {
                code: None,
                message: Some(format!("Failed to acquire read lock: {e}")),
                data: (),
            }))
        })?;
        guard.get(event).cloned()
    };

    if let Some(channels) = channels {
        let value: serde_json::Value = serde_json::from_str(payload).map_err(|e| {
            crate::Error::from(PluginInvokeError::InvokeRejected(ErrorResponse {
                code: None,
                message: Some(format!("Failed to parse payload JSON: {e}")),
                data: (),
            }))
        })?;
        for channel in channels.values() {
            let _ = channel.send(value.clone());
        }
    }
    Ok(())
}

/// Register a channel to receive events for the given event name.
#[tauri::command]
pub fn register_listener(
    event: String,
    handler: tauri::ipc::Channel<serde_json::Value>,
) -> crate::Result<()> {
    let listeners = LISTENERS.get_or_init(|| RwLock::new(HashMap::new()));
    {
        let mut guard = listeners.write().map_err(|e| {
            crate::Error::from(PluginInvokeError::InvokeRejected(ErrorResponse {
                code: None,
                message: Some(format!("Failed to acquire write lock: {e}")),
                data: (),
            }))
        })?;
        guard
            .entry(event)
            .or_default()
            .insert(handler.id(), handler);
    }
    Ok(())
}

/// Remove a previously registered listener by event name and channel ID.
// Tauri commands require owned/deserializable types for args, so `event` must be
// `String` even though the body only borrows it.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn remove_listener(event: String, channel_id: u32) -> crate::Result<()> {
    let listeners = LISTENERS.get().ok_or_else(|| {
        crate::Error::from(PluginInvokeError::InvokeRejected(ErrorResponse {
            code: None,
            message: Some("Listeners not initialized".to_string()),
            data: (),
        }))
    })?;
    {
        let mut guard = listeners.write().map_err(|e| {
            crate::Error::from(PluginInvokeError::InvokeRejected(ErrorResponse {
                code: None,
                message: Some(format!("Failed to acquire write lock: {e}")),
                data: (),
            }))
        })?;
        if let Some(channels) = guard.get_mut(&event) {
            channels.remove(&channel_id);
        }
    }
    Ok(())
}
