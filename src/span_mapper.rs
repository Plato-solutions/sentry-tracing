use sentry::{
    protocol::{self, SpanId, TraceId, Value}
};
use sentry_tracing::extract_span_data;
use tracing_core::{span, Subscriber};
use tracing_subscriber::{
    registry::{LookupSpan, SpanRef},
};
use std::str::FromStr;

pub fn custom_span_mapper<S>(
    span: &SpanRef<S>,
    parent: Option<&protocol::Span>,
    attrs: &span::Attributes,
) -> protocol::Span
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let (description, data) = extract_span_data(attrs);
    // leave the possibility for the user to specify trace_id and span_id.
    // This may be needed when receiving
    // a trace_id and span_id via "sentry-trace" http request header
    let trace_id = match data.get("trace_id") {
        Some(trace_id) => match trace_id {
            Value::String(trace_id) => TraceId::from_str(trace_id).ok(),
            _ => None,
        },
        None => None,
    };
    let trace_id = match trace_id {
        Some(trace_id) => trace_id,
        None => parent
            .map(|parent| parent.trace_id)
            .unwrap_or_else(TraceId::default),
    };
    let span_id = match data.get("span_id") {
        Some(span_id) => match span_id {
            Value::String(span_id) => SpanId::from_str(span_id).ok(),
            _ => None,
        },
        None => None,
    };
    let span_id = match span_id {
        Some(span_id) => span_id,
        None => SpanId::default(),
    };

    protocol::Span {
        trace_id,
        span_id,
        op: Some(span.name().into()),
        description,
        data,
        ..protocol::Span::default()
    }
}