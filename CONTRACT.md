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

## Round 2 obligations

PR-19. Amendment to PR-12. The fixation detector reports one detection for every dominant tool that holds a qualifying span inside a window, and each detection carries the shortest qualifying span for its own tool as its evidence, and no detection merges calls from two tools into one span, and the detections from one window are ordered by the start step of their evidence spans with the tool name breaking a tie between spans that start at the same step.

PR-20. Two judgment points stand after a detector fires and neither point decides whether a stall happened. The severity point takes a request that names the trace and the shape and the pattern and the evidence span of one detection and answers with a score for how deep that stall runs. The recovery point takes a request that names the trace and the rung that fired and the shape and the pattern of the detection it fired on and the events recorded after that rung landed, up to and including the event that triggered the next firing or the last event of the session where no further rung fires, and answers with one choice from recovering and still_stalled and getting_worse. A judgment is recorded as a fixture that pairs one request with one response, and a choice response carries the chosen label and a confidence and a probability for every label, and a score response carries a numeric score and a confidence, and an abstention response carries neither a choice nor a score. Every committed fixture is synthetic and built for this repo and is marked synthetic in its file.

PR-21. The fixture player answers a judgment request only from a recorded fixture whose request matches the request exactly, and where no recorded fixture matches it abstains by returning no answer, and two recorded fixtures that pair the same point with the same request are refused at load as an ambiguous recording, and the player never opens a network connection and never reads a wall clock and the whole judgment seam runs with networking unreachable.

PR-22. Every fixture is validated when the player loads it, and a fixture that breaks the recorded shape is refused at load with the judgment error naming the file and the defect, and a test validates every committed fixture against the same shape so a provider response that changes shape fails loudly in the suite.

PR-23. A judgment answer is trusted only when its confidence reaches 0.6, and an answer below that confidence and an abstention and the absence of a judgment source are all treated the same way, as no judgment at all, and in every one of those cases the ladder and the sandbox behave exactly as they behaved before the seam existed.

PR-24. A trusted severity score is recorded on the intervention record the ladder builds for that detection under the name severity, and it changes nothing else about the record or the session. A trusted recovery choice is recorded on the intervention record for the rung it judges under the name recovery, with the label recorded verbatim. No judgment answer reorders a rung or skips a rung or fires a rung, and the escalation order in PR-6 governs every firing with or without a judgment source.

PR-25. The intervene command accepts an optional fixtures flag naming a fixtures directory, and with the flag the session runs with the fixture player for that directory as its judgment source, and a fixtures directory that cannot be loaded prints one error line and exits with status 2 and no traceback is shown, and without the flag the command prints exactly the bytes it printed before the seam existed.

PR-26. Amendment, added by the Round 2 review. The recorded response shapes are exact. A score response carries only its kind, score, and confidence, a choice response only its kind, choice, confidence, and probabilities, and an abstention response only its kind, and a fixture whose response carries any further field is refused at load under PR-22. A severity score is read on the unit interval, as PR-20's validation already enforces. When a judgment source that is not the fixture player answers, the sandbox checks the answer against the same shape for its point before trusting it, and an answer outside that shape, including an answer of the wrong kind for the point, counts as no judgment at all under PR-23 and is never recorded. Where two spans for one tool are equally short and both qualify under PR-12, the reported span is the one that starts earliest.

## Round 3 obligations

PR-27. Amendment to PR-22 and PR-26. A response whose kind is not a string is a malformed answer, and a fixture carrying such a response is refused at load with the judgment error, and such an answer from a judgment source that is not the fixture player counts as no judgment at all under PR-23, and neither validation nor checking ever raises another error for such a response. A numeric field whose value cannot be converted to a finite float is not a number for the unit interval checks or for the confidence threshold check in PR-23, and it is refused at load and counted as no judgment from any other source on the same terms, and neither validation nor checking nor the threshold check ever raises another error for such a value. The fixture envelope carries exactly the fields synthetic and judgment_point and request and response, and a fixture carrying any other top level field is refused at load with the judgment error naming that field. A choice response carries probabilities that add up to 1 within a tolerance of 0.001, and a choice response whose probabilities add up to a value outside that tolerance is refused at load and counted as no judgment from any other source on the same terms.

PR-28. The looping detector reports matches inside one stall window in a fixed order and segments the window by that order. The detector scans offsets from the start of the window in increasing order, and at one offset it tries pattern lengths from one to four calls in increasing order, and the first pattern that meets PR-1 at the earliest offset and the shortest length is the match reported for that scan. A match counts its repeats maximally, so its evidence span ends at the last call of its last full repeat and no shorter span for the same match is reported. After a match the detector resumes scanning at the first call just past the end of that match, and a pattern that begins inside a reported match is never reported for that window, and matches later in the same window are reported after the resume point on the same terms.

PR-29. Amendment, added by the Round 3 review. Three precision fixes. First, the extra field refusal in the response validator names the first extra field in the order the fields sort by their printed form, so a response carrying extra fields of mixed types, which no JSON file can hold but a foreign source can hand back, is refused or counted as no judgment on the same terms as any other extra field and never raises a comparison error out of validation or checking; PR-26's promise that a malformed answer cannot crash a session covers this shape expressly. Second, PR-28's match order never compares how far a span reaches: the earliest offset wins and the shortest pattern at that offset wins whatever a later or longer pattern would have covered, and a pattern beginning inside a reported span is reported only for the part of it, if any, that still qualifies after scanning resumes past that span. Third, PR-27's probability sum is computed in binary floating point in label order, so at the exact edge of the named tolerance the computed sum decides, as the fleet's other seams have recorded of the same arithmetic.
