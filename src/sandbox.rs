//! The local sandbox harness.
//!
//! A scripted test agent is driven by a fixed script held here.
//! It emits stall events until it receives an intervention it
//! responds to, and then it emits recovery events that make
//! progress and finish. The harness runs the detectors over the
//! session as it grows, fires the ladder in order, and records
//! the whole session as one timeline of events and interventions.
//! Nothing here touches a network, a wall clock, or any agent
//! outside this module, and nothing is left to chance.
//!
//! A session may run with a judgment source, any object that
//! answers judge(point, request) with a recorded response or with
//! no answer. The harness then asks for a severity score before a
//! rung fires and records a trusted score on the intervention
//! record, and asks for a recovery choice over the segment that
//! follows each rung and records a trusted choice on that record.
//! With no judgment source, or where an answer is missing or
//! untrusted, the session record is exactly the record the
//! harness produced before the seam existed.

use crate::detect::detect;
use crate::judgment::{JudgmentSource, check_answer, recovery_request, severity_request, trusted};
use crate::ladder::{LadderError, apply_rung, next_rung};
use crate::pyjson::Value;
use crate::trace::{Event, Trace};

pub const MAX_STEPS: usize = 200;

/// One scripted event before the harness numbers it.
#[derive(Clone, Copy)]
pub struct RawEvent {
    pub tool: &'static str,
    pub call: &'static str,
    pub progress: bool,
}

/// One scripted scenario, the fixed script an agent follows.
#[derive(Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub goal: &'static str,
    pub tools_available: Vec<&'static str>,
    pub stall_events: Vec<RawEvent>,
    pub recovery_events: Vec<RawEvent>,
    pub responds_to: Vec<&'static str>,
}

fn raw(tool: &'static str, call: &'static str, progress: bool) -> RawEvent {
    RawEvent {
        tool,
        call,
        progress,
    }
}

fn scenario(
    name: &'static str,
    goal: &'static str,
    stall_events: Vec<RawEvent>,
    recovery_events: Vec<RawEvent>,
    responds_to: &'static [&'static str],
) -> Scenario {
    Scenario {
        name,
        goal,
        tools_available: vec!["grep", "read", "edit", "test", "finish"],
        stall_events,
        recovery_events,
        responds_to: responds_to.to_vec(),
    }
}

fn loop_stall() -> Vec<RawEvent> {
    vec![raw("grep", "grep(pattern=config, path=src)", false)]
}

fn fixation_stall() -> Vec<RawEvent> {
    vec![
        raw("grep", "grep(pattern=config, path=src)", false),
        raw("grep", "grep(pattern=settings, path=src)", false),
        raw("grep", "grep(pattern=config, path=docs)", false),
        raw("grep", "grep(pattern=conf, path=src)", false),
    ]
}

fn recovery() -> Vec<RawEvent> {
    vec![
        raw("read", "read(path=src/config.py)", true),
        raw("test", "test(target=config)", true),
        raw("finish", "finish(summary=config located)", true),
    ]
}

/// The scripted scenarios, by name.
pub fn scenarios() -> Vec<Scenario> {
    let goal = "Find the config file and report its path";
    vec![
        scenario(
            "loop-clears-on-nudge",
            goal,
            loop_stall(),
            recovery(),
            &["nudge"],
        ),
        scenario(
            "loop-needs-sharper",
            goal,
            loop_stall(),
            recovery(),
            &["sharper_prompt"],
        ),
        scenario(
            "loop-needs-subtask",
            goal,
            loop_stall(),
            recovery(),
            &["smaller_subtask"],
        ),
        scenario(
            "fixation-clears-on-nudge",
            goal,
            fixation_stall(),
            recovery(),
            &["nudge"],
        ),
        scenario("loop-never-clears", goal, loop_stall(), recovery(), &[]),
    ]
}

/// The scenario names in the sorted order the command line
/// offers them in.
pub fn scenario_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = scenarios().iter().map(|item| item.name).collect();
    names.sort();
    names
}

/// A test agent whose behaviour is fixed by its scenario script.
struct ScriptedAgent {
    tools_available: Vec<String>,
    stall: Vec<RawEvent>,
    recovery: Vec<RawEvent>,
    responds_to: Vec<String>,
    recovering: bool,
    index: usize,
    finished: bool,
}

impl ScriptedAgent {
    fn new(scenario: &Scenario) -> ScriptedAgent {
        ScriptedAgent {
            tools_available: scenario
                .tools_available
                .iter()
                .map(|tool| tool.to_string())
                .collect(),
            stall: scenario.stall_events.clone(),
            recovery: scenario.recovery_events.clone(),
            responds_to: scenario
                .responds_to
                .iter()
                .map(|rung| rung.to_string())
                .collect(),
            recovering: false,
            index: 0,
            finished: false,
        }
    }

    fn receive(&mut self, intervention: &Value) {
        let rung = match intervention.get("rung") {
            Some(Value::Str(text)) => text.as_str(),
            _ => return,
        };
        if !self.recovering && self.responds_to.iter().any(|name| name == rung) {
            self.recovering = true;
            self.index = 0;
        }
    }

    fn next_event(&mut self) -> Option<RawEvent> {
        let raw = if self.recovering {
            if self.index >= self.recovery.len() {
                self.finished = true;
                return None;
            }
            self.recovery[self.index]
        } else {
            self.stall[self.index % self.stall.len()]
        };
        self.index += 1;
        if raw.tool == "finish" {
            self.finished = true;
        }
        Some(raw)
    }
}

/// The failure of running a session. An unknown scenario is the
/// usage refusal the command line answers at status 2. A ladder
/// refusal cannot arise from the records the harness itself
/// builds, and the Python harness lets one escape uncaught, so
/// it is its own kind and the command line answers it at
/// status 1.
#[derive(Debug, thiserror::Error)]
pub enum RunSessionError {
    #[error("unknown scenario {0}")]
    UnknownScenario(String),
    #[error("{0}")]
    Ladder(#[from] LadderError),
}

fn int_of(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Int(text)) => text.parse().unwrap_or(0),
        _ => 0,
    }
}

/// Ask about the pending intervention's recovery over the
/// segment that followed it, and record a trusted choice on it.
///
/// The pending record is consumed whatever happens, and it is
/// consumed by the first assessment that follows it, over the
/// segment ending at the step the assessment is called with.
fn assess_recovery(
    judgments: Option<&dyn JudgmentSource>,
    pending: &mut Option<usize>,
    events: &[Value],
    interventions: &mut [Value],
    upto_step: i64,
) {
    let Some(index) = pending.take() else {
        return;
    };
    let Some(source) = judgments else {
        return;
    };
    let record = interventions[index].clone();
    let at_step = int_of(record.get("at_step"));
    let segment: Vec<Value> = events
        .iter()
        .filter(|event| {
            let step = int_of(event.get("step"));
            at_step < step && step <= upto_step
        })
        .cloned()
        .collect();
    if segment.is_empty() {
        return;
    }
    let request = recovery_request(&record, &segment);
    let answer = check_answer("recovery", source.judge("recovery", &request));
    if trusted(answer.as_ref()) {
        let choice = answer
            .and_then(|response| response.get("choice").cloned())
            .unwrap_or(Value::Null);
        interventions[index].set("recovery", choice);
    }
}

/// Run one scripted sandbox session and return its full record.
///
/// The optional judgments source is asked at the two judgment
/// points the contract states. It never changes which rung fires
/// or when, only what the intervention records carry.
pub fn run_session(
    scenario_name: &str,
    judgments: Option<&dyn JudgmentSource>,
) -> Result<Value, RunSessionError> {
    let all = scenarios();
    let Some(scenario) = all.iter().find(|item| item.name == scenario_name) else {
        return Err(RunSessionError::UnknownScenario(scenario_name.to_string()));
    };
    let mut agent = ScriptedAgent::new(scenario);
    let mut events: Vec<Value> = Vec::new();
    let mut interventions: Vec<Value> = Vec::new();
    let mut detections: Vec<Value> = Vec::new();
    let mut last_intervention_step: i64 = -1;
    let mut stalled_out = false;
    let mut pending: Option<usize> = None;

    while !agent.finished && events.len() < MAX_STEPS {
        let Some(raw_event) = agent.next_event() else {
            break;
        };
        let step = events.len() as i64;
        events.push(Value::object(vec![
            ("step".to_string(), Value::int(step)),
            ("tool".to_string(), Value::Str(raw_event.tool.to_string())),
            ("call".to_string(), Value::Str(raw_event.call.to_string())),
            ("progress".to_string(), Value::Bool(raw_event.progress)),
        ]));
        if agent.finished {
            break;
        }
        let trace = Trace {
            trace_id: format!("sandbox-{}", scenario_name),
            goal: scenario.goal.to_string(),
            tools_available: agent.tools_available.clone(),
            events: events
                .iter()
                .map(|event| Event {
                    step: match event.get("step") {
                        Some(Value::Int(text)) => text.clone(),
                        _ => "0".to_string(),
                    },
                    tool: match event.get("tool") {
                        Some(Value::Str(text)) => text.clone(),
                        _ => String::new(),
                    },
                    call: match event.get("call") {
                        Some(Value::Str(text)) => text.clone(),
                        _ => String::new(),
                    },
                    progress: matches!(event.get("progress"), Some(Value::Bool(true))),
                })
                .collect(),
        };
        let fresh = detect(&trace, last_intervention_step + 1);
        if fresh.is_empty() {
            continue;
        }
        let detection = fresh.into_iter().next().expect("a detection stands");
        let rung = next_rung(&detection, &interventions)?;
        let Some(rung) = rung else {
            assess_recovery(judgments, &mut pending, &events, &mut interventions, step);
            stalled_out = true;
            detections.push(detection);
            break;
        };
        assess_recovery(judgments, &mut pending, &events, &mut interventions, step);
        let mut severity: Option<Value> = None;
        if let Some(source) = judgments {
            let answer = check_answer(
                "severity",
                source.judge("severity", &severity_request(&detection)),
            );
            if trusted(answer.as_ref()) {
                severity = answer.and_then(|response| response.get("score").cloned());
            }
        }
        let record = apply_rung(
            &rung,
            &detection,
            scenario.goal,
            Value::int(step),
            &interventions,
            severity,
        )?;
        interventions.push(record);
        detections.push(detection);
        if let Some(last) = interventions.last() {
            agent.receive(last);
        }
        last_intervention_step = step;
        pending = Some(interventions.len() - 1);
    }
    if let Some(last) = events.last() {
        let upto = int_of(last.get("step"));
        assess_recovery(judgments, &mut pending, &events, &mut interventions, upto);
    }
    let stall_cleared = agent.finished && !stalled_out;
    let cleared_by = if stall_cleared && !interventions.is_empty() {
        interventions
            .last()
            .and_then(|record| record.get("rung").cloned())
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    Ok(Value::object(vec![
        (
            "session_id".to_string(),
            Value::Str(format!("sandbox-{}", scenario_name)),
        ),
        ("goal".to_string(), Value::Str(scenario.goal.to_string())),
        ("events".to_string(), Value::List(events)),
        ("detections".to_string(), Value::List(detections)),
        ("interventions".to_string(), Value::List(interventions)),
        ("stall_cleared".to_string(), Value::Bool(stall_cleared)),
        (
            "outcome".to_string(),
            Value::Str(if stall_cleared { "cleared" } else { "stalled" }.to_string()),
        ),
        ("cleared_by".to_string(), cleared_by),
    ]))
}
