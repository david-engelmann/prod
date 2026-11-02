# prod

A cattle prod for stalled agents that escalates in stages, a nudge, a sharper prompt, a smaller subtask, a fork, and only then a kill.

## What this is

Everything else in fleet tooling either waits forever or kills, and stopping a runaway fleet today means killing it, with no graded middle. Prod is the middle as a discipline. Stall detectors watch per failure shape, looping, dithering, context rot, and tool fixation, and an intervention playbook matched to the shape runs up the ladder. The whole rescue is shown as one timeline, so an operator can see which rung worked and teach the ladder from real rescues. The boundary with its siblings is one sentence. Prod is the ladder, deadman is the last rung, cavalry is the log of every time a person had to climb it.

## Status

Round 1 is built and in review. The detectors, the first two ladder rungs, and the sandbox harness are in place against the Round 1 contract. Closed rounds 0 of 100. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
