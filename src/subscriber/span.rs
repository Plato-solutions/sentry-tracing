use crate::converters::ParentSpanIter;
use crate::converters::{default_convert_transaction, default_new_span};
use crate::timings::Timings;
use crate::trace_ext::TraceExt;
use sentry_core::protocol::SpanId;
use sentry_core::Envelope;
use sentry_core::Hub;
use std::str::FromStr;
use std::time::{Instant, SystemTime};
use tracing::Collect;
use tracing::{span, span::Id};
use tracing_subscriber::subscribe::Context;
use tracing_subscriber::Subscribe;

pub struct SpanLayer {
    enabled: bool,
}

impl SpanLayer {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl<C: Collect + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>> Subscribe<C>
    for SpanLayer
{
    fn enabled(&self, _metadata: &tracing::Metadata<'_>, _ctx: Context<'_, C>) -> bool {
        self.enabled
    }
    fn new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, C>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();
        let no_trace_ext = extensions.get_mut::<TraceExt>().is_none();
        // TODO: implement sampling rate
        if no_trace_ext {
            for span_id in ParentSpanIter::new(&ctx, id.clone()) {
                let parent = ctx.span(&span_id).expect("Span not found, this is a bug");
                let parent_extensions = parent.extensions();
                let trace_ext = match parent_extensions.get::<TraceExt>() {
                    Some(trace) => trace,
                    None => {
                        continue;
                    }
                };
                let span = default_new_span(&span, Some(&trace_ext.span), attrs);
                extensions.insert(TraceExt::new(span));
                return;
            }
            let span = default_new_span(&span, None, attrs);
            extensions.insert(TraceExt::new(span));
        }
    }

    /// Notifies this layer that a span with the given ID was entered.
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, C>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        if let Some(trace_ext) = extensions.get_mut::<TraceExt>() {
            let now = Instant::now();
            trace_ext.idle += (now - trace_ext.last).as_nanos() as u64;
            trace_ext.span.start_timestamp = SystemTime::now().into();
            trace_ext.last = now;
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, C>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        if let Some(trace_ext) = extensions.get_mut::<TraceExt>() {
            let now = Instant::now();
            trace_ext.busy += (now - trace_ext.last).as_nanos() as u64;
            trace_ext.last = now;
            trace_ext.span.timestamp = Some(SystemTime::now().into());
            trace_ext.last_sys = SystemTime::now();
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, C>) {
        let span = ctx.span(&id).expect("Span not found, this is a bug");
        let mut extensions = span.extensions_mut();

        let mut trace_ext = match extensions.remove::<TraceExt>() {
            Some(trace_ext) => trace_ext,
            None => return,
        };

        trace_ext.idle += (std::time::Instant::now() - trace_ext.last).as_nanos() as u64;
        let timings = Timings {
            start_time: trace_ext.first,
            end_time: trace_ext.last_sys,
            idle: trace_ext.idle,
            busy: trace_ext.busy,
        };

        // Traverse the parents of this span to attach to the nearest one
        // that has tracing data (spans ignored by the span_filter do not)
        for span_id in ParentSpanIter::new(&ctx, id.clone()) {
            let parent = ctx.span(&span_id).expect("Span not found, this is a bug");
            let mut extensions = parent.extensions_mut();
            if let Some(current_trace_ext) = extensions.get_mut::<TraceExt>() {
                current_trace_ext.spans.extend(trace_ext.spans);

                let span_id = current_trace_ext.span.span_id.to_string();
                trace_ext.span.parent_span_id =
                    Some(SpanId::from_str(&span_id).expect("Bad span id"));
                current_trace_ext.spans.push(trace_ext.span);
                return;
            }
        }
        // If no parent was found, consider this span a
        // transaction root and submit it to Sentry
        let span = &span;
        Hub::with_active(move |hub| {
            let transaction =
                default_convert_transaction(trace_ext.span, span, trace_ext.spans, timings);
            let envelope = Envelope::from(transaction);
            hub.client().unwrap().send_envelope(envelope);
        });
    }
}
