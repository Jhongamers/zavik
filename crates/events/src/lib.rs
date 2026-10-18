//! Crate `devforge-events` — the **EventBus**.
//!
//! Reactive, decoupled communication between the Kernel, Core, and plugins. No
//! Core module calls a presentation layer directly: they all exchange
//! messages through here (ADR 0002).
//!
//! The current implementation is synchronous and thread-safe. Evolving it to
//! `tokio::sync::broadcast` (asynchronous) is transparent to those who merely
//! publish/subscribe to events.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Envelope carried by the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// Domain event type (e.g. `"system_ready"`, `"project_opened"`).
    pub event_type: String,
    /// Event payload as free-form JSON.
    pub payload: serde_json::Value,
}

impl EventEnvelope {
    /// Shortcut to create an event without a payload.
    pub fn new(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            payload: serde_json::Value::Null,
        }
    }
}

/// Subscriber registered on the bus.
type Handler = Arc<dyn Fn(&EventEnvelope) + Send + Sync + 'static>;

/// Central event bus.
#[derive(Default)]
pub struct EventBus {
    handlers: RwLock<Vec<Handler>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a handler that will be called for **every** published event.
    /// The handler filters by the `event_type` it cares about.
    pub fn subscribe<F>(&self, handler: F)
    where
        F: Fn(&EventEnvelope) + Send + Sync + 'static,
    {
        self.handlers
            .write()
            .expect("EventBus: lock de escrita envenenado")
            .push(Arc::new(handler));
    }

    /// Publishes an event to all active subscribers.
    pub fn publish(&self, event: EventEnvelope) {
        let handlers = self
            .handlers
            .read()
            .expect("EventBus: lock de leitura envenenado");
        eprintln!(
            "{}",
            devforge_shared::i18n::t_fmt(
                "EVENTBUS: publicando '{}' para {} assinante(s)",
                &[&event.event_type, &handlers.len().to_string()]
            )
        );
        for handler in handlers.iter() {
            handler(&event);
        }
    }

    /// Number of registered subscribers (useful in tests).
    pub fn subscriber_count(&self) -> usize {
        self.handlers.read().expect("EventBus: lock envenenado").len()
    }
}
