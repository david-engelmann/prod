//! Recorded decision model judgments, played back from fixtures.
//!
//! The roadmap gives westering two judgment points after a detector
//! fires. Severity rates how deep a detected stall runs and
//! answers with a score. Recovery judges the trace segment
//! recorded after a rung landed and answers with one choice from
//! recovering, still_stalled, and getting_worse. No judgment
//! decides whether a stall happened, and no judgment fires,
//! skips, or reorders a rung.
//!
//! A judgment is recorded as a fixture that pairs one request
//! with one response in the shape a decision model returns. A
//! choice response carries the chosen label, a confidence, and a
//! probability for every label. A score response carries a
//! numeric score and a confidence. An abstention response carries
//! neither. The fixture player answers a request only from a
//! recorded fixture whose request matches it exactly, and
//! abstains where no fixture matches. Everything here is
//! offline. The player reads files from one directory and
//! nothing else.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ladder::RUNGS;
use crate::pyjson::{
    self, ParseError, Value, deepest_path, int_text_cmp, py_path_display, py_repr,
};

pub const CONFIDENCE_THRESHOLD: f64 = 0.6;

pub const JUDGMENT_POINTS: [&str; 2] = ["severity", "recovery"];
pub const RECOVERY_LABELS: [&str; 3] = ["recovering", "still_stalled", "getting_worse"];

fn kind_for_point(point: &str) -> Option<&'static str> {
    match point {
        "severity" => Some("score"),
        "recovery" => Some("choice"),
        _ => None,
    }
}

/// A fixture file or fixtures directory does not match the
/// format. The message is the refusal text itself, without the
/// error prefix the command line adds.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct JudgmentError(pub String);

fn fail<T>(source: &str, defect: &str) -> Result<T, JudgmentError> {
    Err(JudgmentError(format!("{} {}", source, defect)))
}

/// The finite float a numeric value converts to, or None.
///
/// A value that cannot convert to a finite float, including an
/// integer too large for a float and a non finite float, is not
/// a number for the checks in this module, and asking never
/// fails.
fn finite_float(value: &Value) -> Option<f64> {
    let converted = match value {
        Value::Int(text) => text.parse::<f64>().ok()?,
        Value::Float(number) => *number,
        _ => return None,
    };
    if converted.is_finite() {
        Some(converted)
    } else {
        None
    }
}

fn check_unit_interval(
    source: &str,
    field: &str,
    value: Option<&Value>,
) -> Result<f64, JudgmentError> {
    let converted = value.and_then(finite_float);
    match converted {
        Some(number) if (0.0..=1.0).contains(&number) => Ok(number),
        _ => fail(
            source,
            &format!("field '{}' is not a number between 0 and 1", field),
        ),
    }
}

fn check_pattern(source: &str, request: &Value) -> Result<(), JudgmentError> {
    match request.get("pattern") {
        Some(Value::List(items)) if items.iter().all(|item| matches!(item, Value::Str(_))) => {
            Ok(())
        }
        _ => fail(source, "request field 'pattern' is not a list of strings"),
    }
}

fn check_trace_id(source: &str, request: &Value) -> Result<(), JudgmentError> {
    match request.get("trace_id") {
        Some(Value::Str(text)) if !text.is_empty() => Ok(()),
        _ => fail(source, "request field 'trace_id' is not a non-empty string"),
    }
}

fn check_shape(source: &str, request: &Value) -> Result<(), JudgmentError> {
    match request.get("shape") {
        Some(Value::Str(text)) if !text.is_empty() => Ok(()),
        _ => fail(source, "request field 'shape' is not a non-empty string"),
    }
}

fn validate_severity_request(source: &str, request: &Value) -> Result<(), JudgmentError> {
    check_trace_id(source, request)?;
    check_shape(source, request)?;
    check_pattern(source, request)?;
    let span = match request.get("evidence_span") {
        Some(span) if span.is_object() => span,
        _ => return fail(source, "request field 'evidence_span' is not an object"),
    };
    let start = match span.get("start_step") {
        Some(Value::Int(text)) => text,
        _ => {
            return fail(
                source,
                "request evidence span field 'start_step' is not an integer",
            );
        }
    };
    let end = match span.get("end_step") {
        Some(Value::Int(text)) => text,
        _ => {
            return fail(
                source,
                "request evidence span field 'end_step' is not an integer",
            );
        }
    };
    if int_text_cmp(end, start) == Ordering::Less {
        return fail(source, "request evidence span runs backwards");
    }
    Ok(())
}

fn validate_recovery_request(source: &str, request: &Value) -> Result<(), JudgmentError> {
    check_trace_id(source, request)?;
    match request.get("rung") {
        Some(Value::Str(text)) if RUNGS.contains(&text.as_str()) => {}
        _ => return fail(source, "request field 'rung' is not a ladder rung"),
    }
    check_shape(source, request)?;
    check_pattern(source, request)?;
    let events = match request.get("events") {
        Some(Value::List(items)) if !items.is_empty() => items,
        _ => return fail(source, "request field 'events' is not a non-empty list"),
    };
    let mut previous: Option<&str> = None;
    for raw in events {
        if !raw.is_object() {
            return fail(source, "request event is not an object");
        }
        let step = match raw.get("step") {
            Some(Value::Int(text)) => text,
            _ => return fail(source, "request event field 'step' is not an integer"),
        };
        match (raw.get("tool"), raw.get("call")) {
            (Some(Value::Str(_)), Some(Value::Str(_))) => {}
            _ => {
                return fail(
                    source,
                    "request event fields 'tool' and 'call' must be strings",
                );
            }
        }
        if !matches!(raw.get("progress"), Some(Value::Bool(_))) {
            return fail(source, "request event field 'progress' is not a boolean");
        }
        if let Some(earlier) = previous
            && int_text_cmp(step, earlier) != Ordering::Greater
        {
            return fail(source, "request event steps are not strictly increasing");
        }
        previous = Some(step);
    }
    Ok(())
}

fn fields_for_kind(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "abstention" => Some(&["kind"]),
        "score" => Some(&["kind", "score", "confidence"]),
        "choice" => Some(&["kind", "choice", "confidence", "probabilities"]),
        _ => None,
    }
}

/// The first key of a record outside the allowed set, in the
/// order a sort by Python repr gives.
fn first_extra_key<'a>(record: &'a Value, allowed: &[&str]) -> Option<&'a str> {
    let mut extra: Vec<&str> = match record {
        Value::Object(entries) => entries
            .iter()
            .map(|(key, _)| key.as_str())
            .filter(|key| !allowed.contains(key))
            .collect(),
        _ => Vec::new(),
    };
    extra.sort_by(|a, b| {
        py_repr(&Value::Str(a.to_string())).cmp(&py_repr(&Value::Str(b.to_string())))
    });
    extra.into_iter().next()
}

fn validate_response(source: &str, point: &str, response: &Value) -> Result<(), JudgmentError> {
    if !response.is_object() {
        return fail(source, "response is not an object");
    }
    let kind = match response.get("kind") {
        Some(Value::Str(text)) => text.clone(),
        _ => return fail(source, "response field 'kind' is not a string"),
    };
    if let Some(fields) = fields_for_kind(&kind)
        && let Some(extra) = first_extra_key(response, fields)
    {
        return fail(
            source,
            &format!(
                "response carries the field {}, which a {} response does not carry",
                py_repr(&Value::Str(extra.to_string())),
                kind
            ),
        );
    }
    if kind == "abstention" {
        return Ok(());
    }
    let expected = kind_for_point(point).unwrap_or_default();
    if kind != expected {
        return fail(
            source,
            &format!(
                "response kind {} does not answer the {} point, which takes a {} or an abstention",
                py_repr(&Value::Str(kind.clone())),
                point,
                expected
            ),
        );
    }
    check_unit_interval(source, "confidence", response.get("confidence"))?;
    if kind == "score" {
        check_unit_interval(source, "score", response.get("score"))?;
        return Ok(());
    }
    match response.get("choice") {
        Some(Value::Str(text)) if RECOVERY_LABELS.contains(&text.as_str()) => {}
        _ => {
            return fail(
                source,
                "response field 'choice' is not one of the recovery labels",
            );
        }
    }
    let probabilities = match response.get("probabilities") {
        Some(probabilities) if probabilities.is_object() => probabilities,
        _ => return fail(source, "response field 'probabilities' is not an object"),
    };
    let covers = match probabilities {
        Value::Object(entries) => {
            entries.len() == RECOVERY_LABELS.len()
                && RECOVERY_LABELS
                    .iter()
                    .all(|label| probabilities.get(label).is_some())
        }
        _ => false,
    };
    if !covers {
        return fail(
            source,
            "response probabilities do not cover exactly the recovery labels",
        );
    }
    let mut total = 0.0f64;
    for label in RECOVERY_LABELS {
        total += check_unit_interval(
            source,
            &format!("probabilities.{}", label),
            probabilities.get(label),
        )?;
    }
    if (total - 1.0).abs() > 0.001 {
        return fail(source, "response probabilities do not add up to 1");
    }
    Ok(())
}

const ENVELOPE_FIELDS: [&str; 4] = ["synthetic", "judgment_point", "request", "response"];

/// One recorded fixture pair, checked against the recorded
/// shape.
///
/// Returns the pair unchanged when it is well formed and raises
/// JudgmentError naming the source and the defect when it is not.
pub fn validate_fixture(data: &Value, source: &str) -> Result<Value, JudgmentError> {
    if !data.is_object() {
        return fail(source, "is not an object");
    }
    if let Some(extra) = first_extra_key(data, &ENVELOPE_FIELDS) {
        return fail(
            source,
            &format!(
                "carries the top level field {}, which the fixture envelope does not carry",
                py_repr(&Value::Str(extra.to_string()))
            ),
        );
    }
    if !matches!(data.get("synthetic"), Some(Value::Bool(true))) {
        return fail(source, "is not marked synthetic");
    }
    let point = match data.get("judgment_point") {
        Some(Value::Str(text)) if JUDGMENT_POINTS.contains(&text.as_str()) => text.clone(),
        _ => return fail(source, "field 'judgment_point' is not a judgment point"),
    };
    let request = match data.get("request") {
        Some(request) if request.is_object() => request,
        _ => return fail(source, "field 'request' is not an object"),
    };
    if point == "severity" {
        validate_severity_request(source, request)?;
    } else {
        validate_recovery_request(source, request)?;
    }
    let response = match data.get("response") {
        Some(response) => response,
        None => return fail(source, "carries no response"),
    };
    validate_response(source, &point, response)?;
    Ok(data.clone())
}

/// The deepest container path the fixture player's request key
/// document carries before its key computation refuses it as
/// nested too deeply, measured on the Python tool. The key
/// document has the point and the request under one root.
const REQUEST_KEY_MAX_DEPTH: usize = 9991;

/// The canonical key a request is filed under, the compact
/// sorted dump of the point and the request under one root.
fn request_key(point: &str, request: &Value) -> Result<String, ParseError> {
    let document = Value::object(vec![
        ("point".to_string(), Value::Str(point.to_string())),
        ("request".to_string(), request.clone()),
    ]);
    if deepest_path(&document) > REQUEST_KEY_MAX_DEPTH {
        return Err(ParseError::TooDeep);
    }
    Ok(pyjson::dumps_compact(&document))
}

/// The failure of reading one fixture file. A judgment refusal
/// is answered on the usual error path. A document nested past a
/// ceiling escapes the Python tool uncaught, so it is its own
/// kind and the command line answers it at status 1.
#[derive(Debug, thiserror::Error)]
pub enum FixtureLoadError {
    #[error("{0}")]
    Judgment(#[from] JudgmentError),
    #[error("fixture file is nested too deeply, {0}")]
    TooDeep(String),
}

/// Read and validate one fixture file at a path the caller has
/// already rendered for the refusal lines.
pub fn load_fixture_file(path: &Path) -> Result<Value, FixtureLoadError> {
    let display = py_path_display(path);
    let name = path
        .file_name()
        .map(|name| py_path_display(Path::new(name)))
        .unwrap_or_default();
    load_fixture_file_rendered(path, &display, &name)
}

fn load_fixture_file_rendered(
    path: &Path,
    display: &str,
    name: &str,
) -> Result<Value, FixtureLoadError> {
    let bytes = std::fs::read(path).map_err(|_| {
        FixtureLoadError::Judgment(JudgmentError(format!(
            "fixture file cannot be read, {}",
            display
        )))
    })?;
    let text = String::from_utf8(bytes).map_err(|_| {
        FixtureLoadError::Judgment(JudgmentError(format!(
            "fixture file is not valid UTF-8, {}",
            display
        )))
    })?;
    let data = pyjson::parse(&text).map_err(|error| match error {
        ParseError::Syntax => FixtureLoadError::Judgment(JudgmentError(format!(
            "fixture file is not valid JSON, {}",
            display
        ))),
        ParseError::TooDeep => FixtureLoadError::TooDeep(display.to_string()),
    })?;
    Ok(validate_fixture(&data, &format!("fixture {}", name))?)
}

/// Join a fixture file name onto a fixtures directory the way
/// pathlib joins it, the rendering the refusal lines carry. The
/// directory's own dot and empty components fold away, parent
/// components stand, and a directory of no parts leaves the name
/// standing alone.
fn pathlib_join(directory: &str, name: &str) -> String {
    // A leading pair of slashes is a root pathlib preserves. A
    // longer run collapses to one slash, as does a single slash.
    let root = if directory.starts_with("//") && !directory.starts_with("///") {
        "//"
    } else if directory.starts_with('/') {
        "/"
    } else {
        ""
    };
    let parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect();
    if parts.is_empty() {
        if root.is_empty() {
            name.to_string()
        } else {
            format!("{}{}", root, name)
        }
    } else {
        format!("{}{}/{}", root, parts.join("/"), name)
    }
}

/// Answers judgment requests from the recorded fixtures in a
/// directory.
///
/// Every JSON file in the directory holds one fixture pair.
/// Pairs are validated as they load, so a response that changes
/// shape fails here, loudly, and never reaches the ladder.
#[derive(Debug)]
pub struct FixturePlayer {
    answers: BTreeMap<String, Value>,
}

impl FixturePlayer {
    pub fn new(directory: &Path) -> Result<FixturePlayer, FixtureLoadError> {
        if !directory.is_dir() {
            return Err(FixtureLoadError::Judgment(JudgmentError(format!(
                "fixtures directory cannot be read, {}",
                py_path_display(directory)
            ))));
        }
        let directory_display = py_path_display(directory);
        let mut entries: Vec<(String, PathBuf, String)> = Vec::new();
        if let Ok(read) = std::fs::read_dir(directory) {
            for entry in read.flatten() {
                let name = entry.file_name();
                let name_bytes = {
                    use std::os::unix::ffi::OsStrExt;
                    name.as_bytes().to_vec()
                };
                if !name_bytes.ends_with(b".json") {
                    continue;
                }
                let name_display = py_path_display(Path::new(&name));
                let joined = pathlib_join(&directory_display, &name_display);
                entries.push((joined, entry.path(), name_display));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut answers: BTreeMap<String, Value> = BTreeMap::new();
        for (joined, path, name) in &entries {
            let pair = load_fixture_file_rendered(path, joined, name)?;
            let point = match pair.get("judgment_point") {
                Some(Value::Str(text)) => text.clone(),
                _ => unreachable!("a validated fixture carries its point"),
            };
            let request = pair
                .get("request")
                .expect("a validated fixture carries its request")
                .clone();
            let key = request_key(&point, &request)
                .map_err(|_| FixtureLoadError::TooDeep(joined.clone()))?;
            if answers.contains_key(&key) {
                return Err(FixtureLoadError::Judgment(JudgmentError(format!(
                    "fixture {} records the same request as an earlier fixture",
                    name
                ))));
            }
            let response = pair
                .get("response")
                .expect("a validated fixture carries its response")
                .clone();
            answers.insert(key, response);
        }
        Ok(FixturePlayer { answers })
    }

    /// The recorded response for this request, or None to
    /// abstain.
    pub fn judge(&self, point: &str, request: &Value) -> Option<Value> {
        // A request nested past the key ceiling makes the Python
        // player raise its recursion error uncaught. Requests at
        // the command line are built from a detection and a
        // session segment and never approach the ceiling, so the
        // port answers the library level case as an abstention.
        let key = request_key(point, request).ok()?;
        self.answers.get(&key).cloned()
    }
}

/// A source of judgments for a session. The fixture player is
/// the recorded source. A test source answers from its own hand.
pub trait JudgmentSource {
    fn judge(&self, point: &str, request: &Value) -> Option<Value>;
}

impl JudgmentSource for FixturePlayer {
    fn judge(&self, point: &str, request: &Value) -> Option<Value> {
        FixturePlayer::judge(self, point, request)
    }
}

/// The answer itself when it is well formed for its point, else
/// None.
///
/// A judgment source that is not the fixture player can hand
/// back anything. An answer outside the recorded shape for its
/// point is treated as no judgment at all, never trusted and
/// never recorded, so a malformed answer cannot crash the
/// session or reach a record.
pub fn check_answer(point: &str, response: Option<Value>) -> Option<Value> {
    let response = response?;
    match validate_response("answer", point, &response) {
        Ok(()) => Some(response),
        Err(_) => None,
    }
}

/// Whether a judgment answer may be used at all.
///
/// An abstention and an answer whose confidence falls below the
/// threshold in the contract both count as no judgment at all.
pub fn trusted(response: Option<&Value>) -> bool {
    let response = match response {
        Some(value) if value.is_object() => value,
        _ => return false,
    };
    match response.get("kind") {
        Some(Value::Str(text)) if text == "score" || text == "choice" => {}
        _ => return false,
    }
    match response.get("confidence").and_then(finite_float) {
        Some(confidence) => confidence >= CONFIDENCE_THRESHOLD,
        None => false,
    }
}

/// The severity request the ladder asks about one detection.
pub fn severity_request(detection: &Value) -> Value {
    Value::object(vec![
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
    ])
}

/// The recovery request for one rung over its post intervention
/// segment.
pub fn recovery_request(record: &Value, segment: &[Value]) -> Value {
    let fired_by = record.get("fired_by").cloned().unwrap_or(Value::Null);
    Value::object(vec![
        (
            "trace_id".to_string(),
            fired_by.get("trace_id").cloned().unwrap_or(Value::Null),
        ),
        (
            "rung".to_string(),
            record.get("rung").cloned().unwrap_or(Value::Null),
        ),
        (
            "shape".to_string(),
            fired_by.get("shape").cloned().unwrap_or(Value::Null),
        ),
        (
            "pattern".to_string(),
            fired_by.get("pattern").cloned().unwrap_or(Value::Null),
        ),
        ("events".to_string(), Value::List(segment.to_vec())),
    ])
}
