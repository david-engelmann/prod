//! The graded rescue ladder, first four rungs.
//!
//! Escalation order is fixed. Nudge fires before sharper prompt,
//! sharper prompt fires before smaller subtask, and smaller
//! subtask fires before fork. A rung fires only when every lower
//! rung has been tried for that stall, where a lower rung tried
//! on the same pattern earlier in the session counts as tried,
//! and a rung that has already been tried for a stall never
//! fires again for it. Every public entry point that builds an
//! intervention enforces the same order against the session
//! history. The ladder as implemented ends at fork. The last
//! rung named by the contract, kill, is not implemented here,
//! and kill belongs to arvin and is never reimplemented here.
//!
//! Records handed to the ladder are validated before they are
//! read. A detection record or a history record that is missing a
//! field the ladder needs is refused with LadderError naming the
//! missing field.

use crate::pyjson::{Value, py_eq, py_str};

pub const RUNGS: [&str; 4] = ["nudge", "sharper_prompt", "smaller_subtask", "fork"];

const DETECTION_FIELDS: [&str; 4] = ["trace_id", "shape", "pattern", "evidence_span"];

/// A rung was asked to fire out of order, or a record was
/// incomplete. The message is the refusal text itself, without
/// the error prefix the command line adds.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct LadderError(pub String);

fn fail(message: String) -> LadderError {
    LadderError(message)
}

fn check_pattern(owner: &str, record: &Value) -> Result<(), LadderError> {
    match record.get("pattern") {
        Some(Value::List(items)) if items.iter().all(|item| matches!(item, Value::Str(_))) => {
            Ok(())
        }
        _ => Err(fail(format!(
            "{} field 'pattern' is not a list of strings",
            owner
        ))),
    }
}

fn validate_detection(detection: &Value) -> Result<(), LadderError> {
    if !detection.is_object() {
        return Err(fail("detection record is not an object".to_string()));
    }
    for field in DETECTION_FIELDS {
        if detection.get(field).is_none() {
            return Err(fail(format!(
                "detection record is missing field '{}'",
                field
            )));
        }
    }
    check_pattern("detection record", detection)?;
    let span = detection
        .get("evidence_span")
        .expect("the field loop found the span");
    if !span.is_object() {
        return Err(fail(
            "detection record field 'evidence_span' is not an object".to_string(),
        ));
    }
    for field in ["start_step", "end_step"] {
        if span.get(field).is_none() {
            return Err(fail(format!(
                "detection record evidence span is missing field '{}'",
                field
            )));
        }
    }
    Ok(())
}

fn validate_history_record(record: &Value) -> Result<(), LadderError> {
    if !record.is_object() {
        return Err(fail("history record is not an object".to_string()));
    }
    if record.get("rung").is_none() {
        return Err(fail("history record is missing field 'rung'".to_string()));
    }
    if !matches!(record.get("rung"), Some(Value::Str(_))) {
        return Err(fail(
            "history record field 'rung' is not a string".to_string(),
        ));
    }
    if record.get("fired_by").is_none() {
        return Err(fail(
            "history record is missing field 'fired_by'".to_string(),
        ));
    }
    let fired_by = record
        .get("fired_by")
        .expect("the presence check found the record");
    if !fired_by.is_object() {
        return Err(fail(
            "history record field 'fired_by' is not an object".to_string(),
        ));
    }
    for field in DETECTION_FIELDS {
        if fired_by.get(field).is_none() {
            return Err(fail(format!(
                "history record fired_by is missing field '{}'",
                field
            )));
        }
    }
    check_pattern("history record fired_by", fired_by)?;
    Ok(())
}

/// Whether two records name the same stall, the pattern key the
/// ladder matches on. The shape compares with Python value
/// equality and the patterns item by item, as a Python tuple
/// comparison does.
fn same_stall(left: &Value, right: &Value) -> bool {
    let shape_matches = match (left.get("shape"), right.get("shape")) {
        (Some(a), Some(b)) => py_eq(a, b),
        _ => false,
    };
    if !shape_matches {
        return false;
    }
    match (left.get("pattern"), right.get("pattern")) {
        (Some(Value::List(a)), Some(Value::List(b))) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| py_eq(x, y))
        }
        _ => false,
    }
}

fn tried_rungs(detection: &Value, history: &[Value]) -> Result<Vec<String>, LadderError> {
    validate_detection(detection)?;
    let mut tried: Vec<String> = Vec::new();
    for record in history {
        validate_history_record(record)?;
        let fired_by = record
            .get("fired_by")
            .expect("the record was validated above");
        if same_stall(fired_by, detection)
            && let Some(Value::Str(rung)) = record.get("rung")
            && !tried.contains(rung)
        {
            tried.push(rung.clone());
        }
    }
    Ok(tried)
}

/// The next rung that may fire for this stall, or None when
/// spent.
pub fn next_rung(detection: &Value, history: &[Value]) -> Result<Option<String>, LadderError> {
    let tried = tried_rungs(detection, history)?;
    for rung in RUNGS {
        if !tried.iter().any(|name| name == rung) {
            return Ok(Some(rung.to_string()));
        }
    }
    Ok(None)
}

fn enforce_order(rung: &str, detection: &Value, history: &[Value]) -> Result<(), LadderError> {
    let Some(index) = RUNGS.iter().position(|name| *name == rung) else {
        return Err(fail(format!("unknown rung {}", rung)));
    };
    let tried = tried_rungs(detection, history)?;
    if tried.iter().any(|name| name == rung) {
        return Err(fail(format!(
            "{} has already been tried for this stall",
            rung
        )));
    }
    let missing: Vec<&str> = RUNGS[..index]
        .iter()
        .filter(|name| !tried.iter().any(|tried_name| tried_name == *name))
        .copied()
        .collect();
    if !missing.is_empty() {
        return Err(fail(format!(
            "{} cannot fire before {} for the same stall",
            rung,
            missing.join(" and ")
        )));
    }
    Ok(())
}

fn field_text(record: &Value, field: &str) -> String {
    match record.get(field) {
        Some(value) => py_str(value),
        None => String::new(),
    }
}

fn pattern_texts(detection: &Value) -> Vec<String> {
    match detection.get("pattern") {
        Some(Value::List(items)) => items.iter().map(py_str).collect(),
        _ => Vec::new(),
    }
}

/// The recorded intervention for one rung firing on one
/// detection.
///
/// The escalation order is enforced against history, so a nudge
/// can be built with no history and a higher rung built without
/// its lower rungs on record is refused. A trusted severity
/// score handed in by the caller is recorded on the record under
/// the name severity and changes nothing else about it.
pub fn build_intervention(
    rung: &str,
    detection: &Value,
    goal: &str,
    at_step: Value,
    history: &[Value],
    severity: Option<Value>,
) -> Result<Value, LadderError> {
    enforce_order(rung, detection, history)?;
    let shape = field_text(detection, "shape");
    let pattern = pattern_texts(detection);
    let message = if rung == "nudge" {
        format!(
            "Stop. Your recent work stalled in the shape {}. Change approach and work toward the goal. The goal is {}.",
            shape, goal
        )
    } else if rung == "sharper_prompt" {
        format!(
            "New instruction. Your previous approach stalled. The pattern was {} in the shape {}, and it made no progress. Do not repeat that pattern. The goal is {}. Take a different first step toward it now.",
            pattern.join(" then "),
            shape,
            goal
        )
    } else if rung == "smaller_subtask" {
        let span = detection
            .get("evidence_span")
            .expect("the detection was validated by the order check");
        format!(
            "Smaller subtask. Your previous approach stalled. The pattern was {} in the shape {}, and it made no progress, and neither a nudge nor a sharper prompt cleared it. The goal is {}. Do only the part of that work inside the stalled span, steps {} to {}, and finish that part before anything else.",
            pattern.join(" then "),
            shape,
            goal,
            field_text(span, "start_step"),
            field_text(span, "end_step")
        )
    } else if rung == "fork" {
        format!(
            "Fork. Your previous approach stalled. The pattern was {} in the shape {}, and it made no progress, and neither a nudge nor a sharper prompt nor a smaller subtask cleared it. The goal is {}. Split the work from here into separate attempts, and run each attempt toward the goal on its own, and keep to the attempt that makes progress.",
            pattern.join(" then "),
            shape,
            goal
        )
    } else {
        // The order check above refuses every word outside
        // RUNGS before the message is built, so this arm is
        // never reached. It stands so every implemented rung
        // names an explicit branch, as PR-44 states.
        return Err(fail(format!("unknown rung {}", rung)));
    };
    let fired_by = Value::object(vec![
        (
            "trace_id".to_string(),
            detection.get("trace_id").cloned().unwrap_or(Value::Null),
        ),
        (
            "shape".to_string(),
            detection.get("shape").cloned().unwrap_or(Value::Null),
        ),
        (
            "pattern".to_string(),
            detection.get("pattern").cloned().unwrap_or(Value::Null),
        ),
        (
            "evidence_span".to_string(),
            detection
                .get("evidence_span")
                .cloned()
                .unwrap_or(Value::Null),
        ),
    ]);
    let mut record = Value::object(vec![
        ("rung".to_string(), Value::Str(rung.to_string())),
        ("fired_by".to_string(), fired_by),
        ("message".to_string(), Value::Str(message)),
        ("at_step".to_string(), at_step),
    ]);
    if let Some(score) = severity {
        record.set("severity", score);
    }
    Ok(record)
}

/// Build an intervention while enforcing the escalation order.
pub fn apply_rung(
    rung: &str,
    detection: &Value,
    goal: &str,
    at_step: Value,
    history: &[Value],
    severity: Option<Value>,
) -> Result<Value, LadderError> {
    build_intervention(rung, detection, goal, at_step, history, severity)
}
