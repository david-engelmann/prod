# Contract - prod

This contract is the executable spec for prod. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

PR-S1. A stall detector names the failure shape it caught, and a run that loops without progress is caught as looping rather than reported as healthy.

PR-S2. Interventions follow the ladder in order, nudge before sharper prompt before smaller subtask before fork before kill, and no rung is skipped without a recorded reason.

PR-S3. Every rescue is shown as one timeline that records each rung tried and which rung worked, and a rescue with no timeline is a defect the tests must catch.

PR-S4. Prod acts only against test agents in local sandboxes during its first slices, and it never nudges, re-prompts, or forks a live run.

PR-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.

## Round 1 obligations

PR-1. The looping detector runs over a recorded trace and catches a span with no progress call in it where one call repeats at least three times in a row or a cycle of two to four calls repeats at least twice in a row, and a span that contains a progress call is never reported as looping.

PR-2. The fixation detector runs over a recorded trace and catches a span of at least four calls with no progress call in it where one tool makes at least four of the calls and at least three quarters of the span and the calls to that tool are not all identical and at least one other available tool goes unused in the span, and a span failing any one of those conditions is never reported as fixation.

PR-3. Every detection names the trace it came from and the pattern it matched and the evidence span as the first and last step of the span, and a detection missing any one of those three is incomplete and the tests reject it.

PR-4. The nudge rung produces a recorded intervention that carries a short corrective message naming the detected shape and pointing the agent back at the goal, and the record names the rung and the detection that fired it.

PR-5. The sharper prompt rung produces a recorded intervention that carries a rewritten instruction naming the stall pattern and restating the goal in full, and the record names the rung and the detection that fired it.

PR-6. Escalation order is fixed with nudge before sharper prompt, and a rung fires only when every lower rung has been tried for that stall or the session record shows the lower rung was already tried on the same pattern earlier in the session, and an attempt to fire sharper prompt for a stall that has no nudge on record is refused.

PR-7. The sandbox harness records the whole session as one timeline of events and interventions, and every intervention in the record carries the step it landed on, so the events before and after each intervention can be separated and the effect of the rung checked from the record alone.

PR-8. The sandbox harness drives only scripted test agents whose behaviour is fixed by a script held in this repo, and no code path in the package opens a network connection or reads a wall clock or uses randomness or addresses a live fleet or a real agent.

PR-9. A scripted agent that recovers after a nudge finishes its session with no sharper prompt rung fired, and a scripted agent that keeps stalling after a nudge escalates to sharper prompt, and a scripted agent that ignores both rungs ends with the stall recorded as not cleared and both rungs on its timeline.

PR-10. The detect and intervene commands print the same bytes for the same input on every run, and the output contains no wall clock time and no random value, and the committed detect output under examples is byte identical to a fresh run of the documented command.

PR-11. Every trace and fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.

## Round 1 review amendments

PR-12. The fixation detector searches subspans inside every no progress window and reports the shortest span that meets every condition in PR-2 for its dominant tool, and the evidence span of a fixation detection is that subspan rather than the whole window, and a qualifying span at the head or the tail of a longer mixed window is reported.

PR-13. Every public entry point that builds an intervention enforces the escalation order in PR-6, and building a sharper prompt record directly with no nudge on record for the same stall is refused with the ladder error, and a rung that has already been tried for a stall is refused at every entry point.

PR-14. The ladder validates every detection record and every history record it is handed, and a detection record missing its trace identifier or its shape or its pattern or its evidence span is refused with the ladder error naming the missing field, and a history record missing its rung or the detection that fired it is refused the same way, and no refusal surfaces as a bare key error.

PR-15. The ladder is spent for a stall once every rung has been tried for that stall, and the entry point that applies a rung refuses a second sharper prompt for the same stall exactly as the next rung lookup reports the ladder spent on the same history.

PR-16. A fixation detection requires at least one available tool that goes unused in the span, and a trace with no available tools never produces a fixation detection, and a detection detail names unused tools only when at least one available tool went unused in the span it describes.

PR-17. A trace whose event steps are not strictly increasing is rejected when the trace is loaded, through the trace error, and no detection from a loaded trace ever carries an evidence span that runs backwards.

PR-18. The hygiene checks read imports and attribute access in the package source structurally, and prose in docstrings and comments is never treated as use of a forbidden capability, and a real import of a network or wall clock or randomness module still fails the checks.
