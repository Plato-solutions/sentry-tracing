use crate::subscriber::breadcrumb::BreadcrumbLayer;
use crate::subscriber::event::EventLayer;
use crate::subscriber::span::SpanLayer;
use tracing_core::Collect;
use tracing_subscriber::subscribe::CollectExt;

pub struct Config {}

/// Composes several subscribers into a Collect implementation
/// event_layer - captures events inside a span
/// span_layer - captures spans as sentry transaction when the root span is closed
/// breadcrumb_layer - captures events as breadcrumbs inside a span
pub struct CollectBuilder {
    event_layer: bool,
    span_layer: bool,
    breadcrumb_layer: bool,
}

impl CollectBuilder {
    pub fn new() -> Self {
        CollectBuilder {
            event_layer: true,
            span_layer: true,
            breadcrumb_layer: true,
        }
    }

    pub fn use_event_layer(mut self, opt: bool) -> Self {
        self.event_layer = opt;
        self
    }

    pub fn use_span_layer(mut self, opt: bool) -> Self {
        self.span_layer = opt;
        self
    }

    pub fn use_breadcrumb_layer(mut self, opt: bool) -> Self {
        self.breadcrumb_layer = opt;
        self
    }

    pub fn build(&self) -> impl Collect {
        let registry = tracing_subscriber::registry();
        registry
            .with(EventLayer::new(self.event_layer))
            .with(BreadcrumbLayer::new(self.breadcrumb_layer))
            .with(SpanLayer::new(self.span_layer))
    }
}
