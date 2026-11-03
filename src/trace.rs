//! Recorded synthetic session traces.
//!
//! A trace is a JSON object with a trace_id, a goal, a list of the
//! tools available to the agent, and a list of events. Every event
//! carries the step number, the tool that was called, the call
//! signature, and whether the call advanced the goal. Event steps
//! must be strictly increasing, so an evidence span over a loaded
//! trace can never run backwards.

use std::cmp::Ordering;
use std::path::Path;

use crate::pyjson::{self, ParseError, Value, int_text_cmp, py_path_display};

/// A trace file or trace object does not match the trace format.
/// The message is the refusal text itself, without the error
/// prefix the command line adds.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct TraceError(pub String);

fn fail(message: &str) -> TraceError {
    TraceError(message.to_string())
}

/// One event in a trace. The step is held as its canonical
/// decimal text, so a step of any length survives exactly, as a
/// Python int does.
#[derive(Clone, Debug)]
pub struct Event {
    pub step: String,
    pub tool: String,
    pub call: String,
    pub progress: bool,
}

impl Event {
    pub fn at(step: i64, tool: &str, call: &str, progress: bool) -> Event {
        Event {
            step: step.to_string(),
            tool: tool.to_string(),
            call: call.to_string(),
            progress,
        }
    }

    /// The event as a JSON value, in the recorded field order.
    pub fn to_value(&self) -> Value {
        Value::object(vec![
            ("step".to_string(), Value::Int(self.step.clone())),
            ("tool".to_string(), Value::Str(self.tool.clone())),
            ("call".to_string(), Value::Str(self.call.clone())),
            ("progress".to_string(), Value::Bool(self.progress)),
        ])
    }
}

/// A recorded trace, validated as it is built from a JSON value.
#[derive(Clone, Debug)]
pub struct Trace {
    pub trace_id: String,
    pub goal: String,
    pub tools_available: Vec<String>,
    pub events: Vec<Event>,
}

impl Trace {
    pub fn from_dict(data: &Value) -> Result<Trace, TraceError> {
        if !data.is_object() {
            return Err(fail("trace is not an object"));
        }
        let trace_id = match data.get("trace_id") {
            Some(Value::Str(text)) if !text.is_empty() => text.clone(),
            _ => return Err(fail("trace_id is not a non-empty string")),
        };
        let goal = match data.get("goal") {
            Some(Value::Str(text)) => text.clone(),
            _ => return Err(fail("goal is not a string")),
        };
        let tools_available = match data.get("tools_available") {
            Some(Value::List(items)) => {
                let mut tools = Vec::new();
                for item in items {
                    match item {
                        Value::Str(text) => tools.push(text.clone()),
                        _ => return Err(fail("tools_available is not a list of strings")),
                    }
                }
                tools
            }
            _ => return Err(fail("tools_available is not a list of strings")),
        };
        let raw_events = match data.get("events") {
            Some(Value::List(items)) => items,
            _ => return Err(fail("events is not a list")),
        };
        let mut events = Vec::new();
        for raw in raw_events {
            if !raw.is_object() {
                return Err(fail("trace event is not an object"));
            }
            let step = match raw.get("step") {
                Some(Value::Int(text)) => text.clone(),
                _ => return Err(fail("event step is not an integer")),
            };
            let (tool, call) = match (raw.get("tool"), raw.get("call")) {
                (Some(Value::Str(tool)), Some(Value::Str(call))) => (tool.clone(), call.clone()),
                _ => return Err(fail("event tool and call must be strings")),
            };
            let progress = match raw.get("progress") {
                Some(Value::Bool(flag)) => *flag,
                _ => return Err(fail("event progress is not a boolean")),
            };
            events.push(Event {
                step,
                tool,
                call,
                progress,
            });
        }
        for pair in events.windows(2) {
            if int_text_cmp(&pair[1].step, &pair[0].step) != Ordering::Greater {
                return Err(fail("event steps are not strictly increasing"));
            }
        }
        Ok(Trace {
            trace_id,
            goal,
            tools_available,
            events,
        })
    }
}

/// The failure of reading a trace file. A trace refusal is
/// answered on the usual error path. A document nested past the
/// reader ceiling escapes the Python tool uncaught, so it is its
/// own kind and the command line answers it at status 1.
#[derive(Debug, thiserror::Error)]
pub enum LoadTraceError {
    #[error("{0}")]
    Trace(#[from] TraceError),
    #[error("trace file is nested too deeply, {0}")]
    TooDeep(String),
}

/// Read and validate one trace file.
pub fn load_trace(path: &Path) -> Result<Trace, LoadTraceError> {
    let display = py_path_display(path);
    let bytes = std::fs::read(path).map_err(|_| {
        LoadTraceError::Trace(TraceError(format!(
            "trace file cannot be read, {}",
            display
        )))
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        LoadTraceError::Trace(TraceError(format!(
            "trace file is not valid UTF-8, {}",
            display
        )))
    })?;
    let data = pyjson::parse(&text).map_err(|error| match error {
        ParseError::Syntax => LoadTraceError::Trace(TraceError(format!(
            "trace file is not valid JSON, {}",
            display
        ))),
        ParseError::TooDeep => LoadTraceError::TooDeep(display.clone()),
    })?;
    Ok(Trace::from_dict(&data)?)
}
