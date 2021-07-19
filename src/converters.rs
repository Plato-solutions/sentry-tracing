use crate::field_visitor::{FieldVisitor, FieldVisitorConfig, FieldVisitorResult};
use crate::timings::Timings;
use crate::trace_ext::TraceExt;
use sentry_core::protocol::{SpanId, SpanStatus, TraceId};
use std::collections::BTreeMap;
use tracing_core::span::Id;
use tracing_core::Collect;
use tracing_subscriber::registry::{LookupSpan, SpanRef};
use tracing_subscriber::subscribe::Context;

use protocol::TraceContext;
use sentry_backtrace::current_stacktrace;
use sentry_core::{
    protocol::Value,
    protocol::{self, Event, Exception, Transaction},
    types::Uuid,
    Breadcrumb,
};
use tracing::span::Attributes;

/// Converts tracing level from tracing crate to level type from sentry crate
fn convert_tracing_level(level: &tracing::Level) -> sentry_core::Level {
    match level {
        &tracing::Level::ERROR => sentry_core::Level::Error,
        &tracing::Level::WARN => sentry_core::Level::Warning,
        &tracing::Level::INFO => sentry_core::Level::Info,
        &tracing::Level::DEBUG | &tracing::Level::TRACE => sentry_core::Level::Debug,
    }
}

/// Strips ansi color escape codes from string, or returns the
/// original string if there was problem performing the strip.
#[cfg(features = "strip-ansi-escapes")]
pub fn strip_ansi_codes_from_string(string: &str) -> String {
    if let Ok(stripped_bytes) = strip_ansi_escapes::strip(string.as_bytes()) {
        if let Ok(stripped_string) = std::str::from_utf8(&stripped_bytes) {
            return stripped_string.to_owned();
        }
    }

    string.to_owned()
}

/// Creates a breadcrumb from a given tracing event.
pub fn breadcrumb_from_event(
    event: &tracing::Event<'_>,
    visitor_config: FieldVisitorConfig,
) -> Breadcrumb {
    let mut visitor_result = FieldVisitorResult::default();
    let mut visitor = FieldVisitor::new(visitor_config, &mut visitor_result);

    event.record(&mut visitor);

    Breadcrumb {
        ty: "log".into(),
        level: convert_tracing_level(event.metadata().level()),
        category: Some(event.metadata().target().into()),
        message: visitor_result.event_message,
        data: visitor_result.json_values,
        ..Default::default()
    }
}

pub fn default_convert_breadcrumb<S>(event: &tracing::Event<'_>, _ctx: Context<S>) -> Breadcrumb {
    breadcrumb_from_event(
        event,
        FieldVisitorConfig {
            event_message_field: None,
            #[cfg(features = "strip-ansi-escapes")]
            strip_ansi_escapes: true,
        },
    )
}

/// Creates an event from a given log record.
///
/// If `attach_stacktraces` is set to `true` then a stacktrace is attached
/// from the current frame.
pub fn convert_tracing_event<C: Collect + for<'a> LookupSpan<'a>>(
    event: &tracing::Event<'_>,
    ctx: Context<C>,
    attach_stacktraces: bool,
    visitor_config: FieldVisitorConfig,
) -> Event<'static> {
    let mut visitor_result = FieldVisitorResult::default();
    let mut visitor = FieldVisitor::new(visitor_config, &mut visitor_result);
    event.record(&mut visitor);

    let exception = if !visitor_result.expections.is_empty() {
        visitor_result.expections
    } else {
        vec![Exception {
            ty: event.metadata().name().into(),
            value: visitor_result.event_message.clone(),
            stacktrace: if attach_stacktraces {
                current_stacktrace()
            } else {
                None
            },
            module: event.metadata().module_path().map(String::from),
            ..Default::default()
        }]
    };

    let mut result = Event {
        logger: Some("sentry-tracing".into()),
        level: convert_tracing_level(event.metadata().level()),
        message: visitor_result.event_message,
        exception: exception.into(),
        extra: visitor_result.json_values,
        ..Default::default()
    };

    let parent = event
        .parent()
        .and_then(|id| ctx.span(id))
        .or_else(|| ctx.lookup_current());
    let span_id = parent.map(|span| span.id());
    match span_id {
        Some(id) => {
            let root_span = ParentSpanIter::root_span(&ctx, id);
            let extensions = root_span.extensions();
            if let Some(trace) = extensions.get::<TraceExt>() {
                // Trace Id will be obtained from the root span
                let context = protocol::Context::from(TraceContext {
                    span_id: trace.span.span_id,
                    trace_id: trace.span.trace_id,
                    ..TraceContext::default()
                });
                result.contexts.insert(context.type_name().into(), context);
                result.transaction = Some(root_span.name().to_owned());
            }
        }
        None => {}
    }

    result
}

pub fn default_convert_event<C: Collect + for<'a> LookupSpan<'a>>(
    event: &tracing::Event<'_>,
    ctx: Context<C>,
) -> Event<'static> {
    convert_tracing_event(
        event,
        ctx,
        true,
        FieldVisitorConfig {
            event_message_field: None,
            #[cfg(features = "strip-ansi-escapes")]
            strip_ansi_escapes: true,
        },
    )
}

pub fn default_new_span<C: Collect + for<'a> LookupSpan<'a>>(
    span: &SpanRef<C>,
    parent: Option<&protocol::Span>,
    attrs: &Attributes,
) -> protocol::Span {
    let mut result = FieldVisitorResult::default();

    let mut visitor = FieldVisitor::new(
        FieldVisitorConfig {
            #[cfg(features = "strip-ansi-escapes")]
            strip_ansi_escapes: true,
            event_message_field: None,
        },
        &mut result,
    );

    attrs.record(&mut visitor);
    // Prioritize trace_id from the parent span, allowing us to only set trace_id for the root span
    let trace_id = if let Some(parent) = parent {
        parent.trace_id
    } else {
        result.trace_id.unwrap_or_else(|| TraceId::default())
    };

    let span_id = result.span_id.unwrap_or_else(|| SpanId::default());
    protocol::Span {
        span_id: span_id,
        trace_id: trace_id,
        op: Some(span.name().into()),
        description: result.event_message,
        data: result.json_values,
        status: if result.expections.is_empty() {
            Some(SpanStatus::Ok)
        } else {
            Some(SpanStatus::InternalError)
        },
        ..protocol::Span::default()
    }
}

pub fn default_on_close(span: &mut protocol::Span, timings: Timings) {
    span.data
        .insert(String::from("busy"), Value::Number(timings.busy.into()));

    span.data
        .insert(String::from("idle"), Value::Number(timings.idle.into()));

    span.timestamp = Some(timings.end_time.into());
}

pub fn default_convert_transaction<C: Collect + for<'a> LookupSpan<'a>>(
    sentry_span: protocol::Span,
    tracing_span: &SpanRef<C>,
    spans: Vec<protocol::Span>,
    timings: Timings,
) -> Transaction<'static> {
    let mut contexts = BTreeMap::new();

    contexts.insert(
        String::from("trace"),
        protocol::Context::Trace(Box::new(TraceContext {
            span_id: sentry_span.span_id,
            trace_id: sentry_span.trace_id,
            parent_span_id: sentry_span.parent_span_id,
            op: Some(tracing_span.name().into()),
            description: sentry_span.description.clone(),
            status: sentry_span.status.clone(),
        })),
    );
    Transaction {
        event_id: Uuid::new_v4(),
        name: Some(tracing_span.name().into()),
        start_timestamp: timings.start_time.into(),
        timestamp: Some(timings.end_time.into()),
        spans,
        contexts,
        ..Transaction::default()
    }
}

pub struct ParentSpanIter<
    'a,
    C: Collect + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
> {
    next_id: Id,
    ctx: &'a Context<'a, C>,
}

impl<'a, C: Collect + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>>
    ParentSpanIter<'a, C>
{
    pub fn new(ctx: &'a Context<'a, C>, id: Id) -> Self {
        ParentSpanIter { next_id: id, ctx }
    }

    pub fn root_span(ctx: &'a Context<'a, C>, id: Id) -> SpanRef<'a, C> {
        let mut span = ctx.span(&id).expect("no span for id");
        while let Some(parent) = span.parent() {
            span = parent;
        }
        span
    }
}

impl<'a, C: Collect + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>> Iterator
    for ParentSpanIter<'a, C>
{
    type Item = Id;
    fn next(&mut self) -> std::option::Option<<Self as std::iter::Iterator>::Item> {
        let span = self.ctx.span(&self.next_id).expect("no span for id");
        let p = span.parent();
        match p {
            Some(parent_span) => {
                self.next_id = parent_span.id();
                return Some(parent_span.id());
            }
            None => None,
        }
    }
}
