# prod

A cattle prod for stalled agents that escalates in stages, a nudge, a sharper prompt, a smaller subtask, a fork, and only then a kill.

## What this is

Everything else in fleet tooling either waits forever or kills, and stopping a runaway fleet today means killing it, with no graded middle. Prod is the middle as a discipline. Stall detectors watch per failure shape, looping, dithering, context rot, and tool fixation, and an intervention playbook matched to the shape runs up the ladder. The whole rescue is shown as one timeline, so an operator can see which rung worked and teach the ladder from real rescues. The boundary with its siblings is one sentence. Prod is the ladder, deadman is the last rung, cavalry is the log of every time a person had to climb it.

## Status

Closed rounds 1 of 100. Round 1 landed in PR 2 and is recorded closed here. The detectors for looping and tool fixation, the first two ladder rungs with the escalation order enforced at every entry point, and the scripted sandbox harness are in place under contract obligations PR-1 to PR-18. The round closed after the independent red team, the two-axis review, the CodeRabbit pass, and the fixes for every finding they raised. Part of the impatient programming portfolio, in the Fleet Operations arc.

## Licence

MIT. See LICENSE.
