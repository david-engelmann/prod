//! A cattle prod for stalled agents. It escalates in stages when an agent stalls: a nudge, a sharper prompt, a smaller subtask, a fork of the work, and only then a kill.

pub mod detect;
pub mod judgment;
pub mod ladder;
pub mod pyjson;
pub mod sandbox;
pub mod trace;

pub use detect::detect;
pub use judgment::{
    CONFIDENCE_THRESHOLD, FixturePlayer, JudgmentError, check_answer, trusted, validate_fixture,
};
pub use ladder::{LadderError, apply_rung, build_intervention, next_rung};
pub use sandbox::{Scenario, run_session, scenario_names, scenarios};
pub use trace::{Event, Trace, TraceError, load_trace};
