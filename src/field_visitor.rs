use sentry_core::event_from_error;
use sentry_core::protocol::{Exception, SpanId, TraceId, Value};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::str::FromStr;
use tracing::field::Field;

/// Configures how sentry event and span data is recorded
// from tracing event and spans attributes
#[derive(Clone, Copy)]
pub struct FieldVisitorConfig<'a> {
    /// If set to true, ansi escape sequences will be stripped from
    /// string values, and formatted error/debug values.
    #[cfg(features = "strip-ansi-escapes")]
    pub strip_ansi_escapes: bool,

    /// If `Some`, values for tracing events with the field name
    /// matching what is specified here will be included as the event
    /// message string.
    pub event_message_field: Option<&'a str>,
}

#[derive(Default)]
pub(crate) struct FieldVisitorResult {
    pub(crate) event_message: Option<String>,
    pub(crate) json_values: BTreeMap<String, Value>,
    pub(crate) expections: Vec<Exception>,
    pub(crate) trace_id: Option<TraceId>,
    pub(crate) span_id: Option<SpanId>,
}

pub(crate) struct FieldVisitor<'a> {
    config: FieldVisitorConfig<'a>,
    result: &'a mut FieldVisitorResult,
}

impl<'a> FieldVisitor<'a> {
    pub(crate) fn new(config: FieldVisitorConfig<'a>, result: &'a mut FieldVisitorResult) -> Self {
        Self { config, result }
    }

    fn record_json_value(&mut self, field: &Field, json_value: Value) {
        self.result
            .json_values
            .insert(field.name().to_owned(), json_value);
    }

    /// Try to record this field as the `event_type`, returns true if the field was
    /// inserted and false if the value was discarded
    fn try_record_event_message(&mut self, field: &Field, value: impl Display) -> bool {
        if let Some(event_message_field) = self.config.event_message_field {
            if field.name() == event_message_field {
                self.result.event_message = Some(value.to_string());
                return true;
            }
        }

        false
    }
}

impl<'a> tracing::field::Visit for FieldVisitor<'a> {
    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &Field, value: i64) {
        if !self.try_record_event_message(field, value) {
            self.record_json_value(field, Value::Number(value.into()));
        }
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &Field, value: u64) {
        if !self.try_record_event_message(field, value) {
            self.record_json_value(field, Value::Number(value.into()));
        }
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &Field, value: bool) {
        if !self.try_record_event_message(field, value) {
            self.record_json_value(field, Value::Bool(value));
        }
    }

    /// Visit an `&str` value.
    fn record_str(&mut self, field: &Field, value: &str) {
        #[cfg(features = "strip-ansi-escapes")]
        let value = if self.config.strip_ansi_escapes {
            strip_ansi_codes_from_string(&value)
        } else {
            value.to_owned()
        };
        // Handle special case for trace_id
        if field.name() == "trace_id" {
            self.result.trace_id = TraceId::from_str(value).ok();
            return;
        }
        // Handle special case for span_id
        if field.name() == "span_id" {
            self.result.span_id = SpanId::from_str(value).ok();
            return;
        }

        if !self.try_record_event_message(field, &value) {
            self.record_json_value(field, Value::String(value.into()));
        }
    }

    /// Visit a type that implements `std::error::Error`.
    fn record_error(&mut self, _field: &Field, value: &(dyn Error + 'static)) {
        // As exception_from_error is not public, this calls event_from_error
        // instead and extract the Exception struct from the resulting Event
        let event = event_from_error(value);
        for exception in event.exception {
            self.result.expections.push(exception);
        }
    }

    /// Visit a type that implements `std::fmt::Debug`.
    #[cfg_attr(not(features = "strip-ansi-escapes"), allow(unused_mut))]
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        let mut formatted_value = format!("{:?}", value);

        #[cfg(features = "strip-ansi-escapes")]
        if self.config.strip_ansi_escapes {
            formatted_value = strip_ansi_codes_from_string(&formatted_value)
        }

        if !self.try_record_event_message(field, &formatted_value) {
            self.record_json_value(field, Value::String(formatted_value));
        }
    }
}
