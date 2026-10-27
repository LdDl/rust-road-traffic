use crate::violations::event::ViolationEvent;
use tracing::{info, warn};

pub trait EventSink: Send + Sync {
    fn publish(&self, event: &ViolationEvent);
}

pub struct ConsoleSink;

impl EventSink for ConsoleSink {
    fn publish(&self, event: &ViolationEvent) {
        match serde_json::to_string(event) {
            Ok(json) => info!(json = %json, "violation event"),
            Err(err) => warn!(
                target: "violation_event",
                error = %err,
                "Failed to serialize violation event"
            ),
        }
    }
}