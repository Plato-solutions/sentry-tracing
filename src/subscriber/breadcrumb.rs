use crate::converters::default_convert_breadcrumb;
use sentry_core::add_breadcrumb;
use tracing::Collect;
use tracing_core::Event;
use tracing_subscriber::subscribe::Context;
use tracing_subscriber::Subscribe;

pub struct BreadcrumbLayer {
    enabled: bool,
}

impl BreadcrumbLayer {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl<C: Collect + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>> Subscribe<C>
    for BreadcrumbLayer
{
    fn enabled(&self, _metadata: &tracing::Metadata<'_>, _ctx: Context<'_, C>) -> bool {
        self.enabled
    }
    /// Notifies this layer that an event has occurred.
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, C>) {
        add_breadcrumb(|| default_convert_breadcrumb(event, ctx));
    }
}
