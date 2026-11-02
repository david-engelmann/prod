# Contract - prod

This contract is the executable spec for prod. Every obligation below is stated so it can fail. The full contract is the first build act of this repo and it extends the stub obligations written here.

## Stub obligations

PR-S1. A stall detector names the failure shape it caught, and a run that loops without progress is caught as looping rather than reported as healthy.

PR-S2. Interventions follow the ladder in order, nudge before sharper prompt before smaller subtask before fork before kill, and no rung is skipped without a recorded reason.

PR-S3. Every rescue is shown as one timeline that records each rung tried and which rung worked, and a rescue with no timeline is a defect the tests must catch.

PR-S4. Prod acts only against test agents in local sandboxes during its first slices, and it never nudges, re-prompts, or forks a live run.

PR-S5. Every fixture and example in this repo is synthetic and built for this repo, and no real trace or session data appears anywhere in the tree.
