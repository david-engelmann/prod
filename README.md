# prod

A cattle prod for stalled agents that escalates in stages, a nudge, a sharper prompt, a smaller subtask, a fork, and only then a kill.

## What this is

Everything else in fleet tooling either waits forever or kills, and stopping a runaway fleet today means killing it, with no graded middle. Prod is the middle as a discipline. Stall detectors watch per failure shape, looping, dithering, context rot, and tool fixation, and an intervention playbook matched to the shape runs up the ladder. The whole rescue is shown as one timeline, so an operator can see which rung worked and teach the ladder from real rescues. The boundary with its siblings is one sentence. Prod is the ladder, deadman is the last rung, cavalry is the log of every time a person had to climb it.

## Status

Closed rounds 5 of 100. Round 2 landed in PR 5 and is recorded closed here. Round 3 landed in PR 7 and is recorded closed here. Round 4 landed in PR 9 and is recorded closed here, and it adds the smaller subtask rung to the ladder, so a stall that survives the nudge and the sharper prompt is handed a smaller next subtask bounded to its evidence span. The fixation detector now reports one detection per dominant tool in a window, and the decision model fixture seam stands behind the ladder under contract obligations PR-1 to PR-50. The seam is recorded fixtures and a fixture player only, built and tested offline with no live calls, and the detectors and the ladder behave exactly as they did in Round 1 wherever no trusted judgment answers. Round 5 landed in PR 11 and is recorded closed here. It ports prod to a Rust crate with no change to the contracted behavior, proven byte for byte against the Python implementation it replaces over the committed scenarios, the committed fixtures, and a generated hostile corpus, with the two readings the contract records standing as the only exceptions. A lone surrogate escape in a JSON string reads as the replacement character, and the failures the Python tool never catches are answered in the tool's own one line discipline at the same status. The Python is removed in the same slice. Round 6 adds the fork rung to the ladder, so a stall that survives the nudge and the sharper prompt and the smaller subtask is handed a directive to split the work from the stalled point into separate attempts and keep to the attempt that makes progress, and the recovery judgment point answers a fork intervention on the same terms as the three earlier rungs. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
