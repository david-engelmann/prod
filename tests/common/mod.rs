//! Shared helpers for the ported test corpus.

#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use westering::pyjson::{self, Value, py_eq};
use westering::trace::Trace;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn value(text: &str) -> Value {
    pyjson::parse(text).expect("test fixture parses")
}

pub fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value.get(name).unwrap_or(&Value::Null)
}

pub fn list<'a>(value: &'a Value, name: &str) -> &'a Vec<Value> {
    field(value, name).as_list().expect("field is a list")
}

pub fn text_of<'a>(value: &'a Value, name: &str) -> &'a str {
    match field(value, name) {
        Value::Str(text) => text,
        _ => panic!("field {} is a string", name),
    }
}

pub fn int_of(value: &Value, name: &str) -> i64 {
    match field(value, name) {
        Value::Int(text) => text.parse().expect("field is a small integer"),
        _ => panic!("field {} is an integer", name),
    }
}

/// Assert two values are equal the way Python dict and list
/// equality would judge them.
pub fn assert_py_eq(left: &Value, right: &Value) {
    assert!(
        py_eq(left, right),
        "values differ\nleft:  {}\nright: {}",
        left,
        right
    );
}

/// The value with one top level field removed, the shape of the
/// Python dict comprehension the record comparisons use.
pub fn without(value: &Value, key: &str) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(name, _)| name != key)
                .cloned()
                .collect(),
        ),
        _ => panic!("value is an object"),
    }
}

/// The value with several top level fields removed.
pub fn without_many(value: &Value, keys: &[&str]) -> Value {
    match value {
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(name, _)| !keys.contains(&name.as_str()))
                .cloned()
                .collect(),
        ),
        _ => panic!("value is an object"),
    }
}

pub const TOOLS: [&str; 5] = ["grep", "read", "edit", "test", "finish"];
pub const GOAL: &str = "Find the config file and report its path";

/// One trace event as a JSON value, the trace_helpers builder.
pub fn ev(step: i64, tool: &str, call: &str, progress: bool) -> Value {
    Value::object(vec![
        ("step".to_string(), Value::int(step)),
        ("tool".to_string(), Value::Str(tool.to_string())),
        ("call".to_string(), Value::Str(call.to_string())),
        ("progress".to_string(), Value::Bool(progress)),
    ])
}

/// A trace document as a JSON value.
pub fn trace_doc(events: &[Value], trace_id: &str, tools: &[&str], goal: &str) -> Value {
    Value::object(vec![
        ("trace_id".to_string(), Value::Str(trace_id.to_string())),
        ("goal".to_string(), Value::Str(goal.to_string())),
        (
            "tools_available".to_string(),
            Value::List(
                tools
                    .iter()
                    .map(|tool| Value::Str(tool.to_string()))
                    .collect(),
            ),
        ),
        ("events".to_string(), Value::List(events.to_vec())),
    ])
}

/// A validated trace over the standard tools and goal.
pub fn make_trace(events: &[Value]) -> Trace {
    make_trace_full(events, "trace-test", &TOOLS, GOAL)
}

pub fn make_trace_full(events: &[Value], trace_id: &str, tools: &[&str], goal: &str) -> Trace {
    Trace::from_dict(&trace_doc(events, trace_id, tools, goal)).expect("trace is valid")
}

/// The detections of one shape, in order.
pub fn detections_of(found: &[Value], shape: &str) -> Vec<Value> {
    found
        .iter()
        .filter(|item| matches!(item.get("shape"), Some(Value::Str(text)) if text == shape))
        .cloned()
        .collect()
}

/// The evidence span of a detection as a step pair.
pub fn span_of(detection: &Value) -> (i64, i64) {
    let span = detection
        .get("evidence_span")
        .expect("detection carries a span");
    (int_of(span, "start_step"), int_of(span, "end_step"))
}

/// The pattern of a detection as text items.
pub fn pattern_of(detection: &Value) -> Vec<String> {
    list(detection, "pattern")
        .iter()
        .map(|item| match item {
            Value::Str(text) => text.clone(),
            _ => panic!("pattern item is a string"),
        })
        .collect()
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn temp_dir(tag: &str) -> PathBuf {
    let number = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "westering-test-{}-{}-{}",
        std::process::id(),
        tag,
        number
    ));
    std::fs::create_dir_all(&dir).expect("temp dir created");
    dir
}

pub struct CliOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_cli(args: &[&str], cwd: &std::path::Path) -> CliOutput {
    let output = Command::new(env!("CARGO_BIN_EXE_westering"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("binary runs");
    CliOutput {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

/// Run the binary at the repo root, the working directory the
/// Python end to end tests use.
pub fn run_cli_at_root(args: &[&str]) -> CliOutput {
    run_cli(args, &root())
}

pub fn write_json(path: &std::path::Path, data: &Value) {
    let text = pyjson::dumps_compact(data);
    std::fs::write(path, text).expect("file written");
}
