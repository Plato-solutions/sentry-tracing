# sentry-tracing
Show the ways in which interconnected systems interact with distributed tracing extension for Sentry.

### Actix-web example
Usage example of sentry-tracing with ability to specify trace_id for a span in case the trace starts on the external system.

Spans will be created for `first` and `second` functions in `handlers.rs` because of `tracing::instrument` attribute. They will both have same parent span, with `trace_id` and `span_id` set to values from `sentry-trace` header.
