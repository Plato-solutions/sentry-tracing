use crate::converters::default_convert_event;
use sentry_core::capture_event;
use tracing::Collect;
use tracing_core::Event;
use tracing_subscriber::subscribe::Context;
use tracing_subscriber::Subscribe;

pub struct EventLayer {
    enabled: bool,
}

impl EventLayer {
    pub fn new(enabled: bool) -> Self {
        EventLayer { enabled }
    }
}

impl<C: Collect + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>> Subscribe<C>
    for EventLayer
{
    fn enabled(&self, _metadata: &tracing::Metadata<'_>, _ctx: Context<'_, C>) -> bool {
        self.enabled
    }
    /// Notifies this layer that an event has occurred.
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, C>) {
        capture_event(default_convert_event(event, ctx));
    }
}
