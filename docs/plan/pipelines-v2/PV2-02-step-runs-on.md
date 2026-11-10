# PV2-02 · Core: a step's connection and effort (*Runs on*)

- **Status:** needs decision — do not start
- **Kind:** core
- **Depends on:** the person's decision

## Why it waits

The design gives a step a *Runs on* field (P2 `u2rYEH`, P10 `HjgZ8`) that opens the compact picker (K1 `lCApz`):
connection, model and effort. Today a step overrides only the model: `StepKind::Agent { model: Option<ModelId>, … }`
(`crates/core/src/resource/pipeline/mod.rs`); the connection comes from the step's agent profile and the effort from
the profile or the CLI.

## Decision needed

1. May a step run on another connection than its profile's (of the same CLI)? The design shows the profile's CLI's
   connections as choosable and other CLIs' at half. If not, the picker's rail shows only the profile's connection.
2. Does a step override the effort, as it does the model?

## Scope once decided

Add the chosen overrides to the step (with serde defaults so published versions read as before), validation against
the profile's CLI, the run's resolution order (step, then profile, then CLI default), the `save_pipeline_draft` and
`get_pipeline` fields, and tests in `crates/core/tests/` and the workspace. No UI (PV2-08).
