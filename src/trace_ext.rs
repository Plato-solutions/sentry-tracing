use sentry_core::protocol::Span;
use std::time::*;

pub(crate) struct TraceExt {
    pub(crate) span: Span,
    pub(crate) spans: Vec<Span>,

    // From the tracing-subscriber implementation of span timings,
    // with additional SystemTime informations to reconstruct the UTC
    // times needed by Sentry
    pub(crate) idle: u64,
    pub(crate) busy: u64,
    pub(crate) last: Instant,
    pub(crate) first: SystemTime,
    pub(crate) last_sys: SystemTime,
}

impl TraceExt {
    pub fn new(span: Span) -> Self {
        TraceExt {
            span,
            spans: Vec::new(),
            idle: 0,
            busy: 0,
            last: Instant::now(),
            first: SystemTime::now(),
            last_sys: SystemTime::now(),
        }
    }
}
