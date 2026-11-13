# OA-02 · The event's line says what happens to a turn that failed

- **Status:** blocked (waits for the frames, `frames.md` A)
- **Kind:** design + UI
- **Depends on:** OA-01, the person's frames
- **Branch:** —
- **Tokens:** —

## Goal

The conversation says what is happening to an event whose turn failed: when it is tried again, which try it is, and
that it was left to the person. (The clock that starts the tries was built with OA-01.)

## Design first

Frames on `Orchestrator WIP` `mPw5l`, beside the tool request's states: an event's row *failed · trying again at
14:05 (2 of 4)*, *left to you after 4 tries* with **Try again**, and the same under a held event (OA-03 reuses them).
Dark and light. No frame, no build.

## Change

- The event rows of `crates/desktop-app/src/screens/orchestrator/` read the tries OA-01 keeps. **Try again** is the
  person's: it starts one more turn now, also after the cap, and synthetic input is refused.
- Strings in `crates/desktop-app/assets/locales/*/orchestrator.json`, every locale.

## Edge cases

- The project closes while a try is due: it starts at the next open, once, not once per missed time.
- The clock and a state change wake the follower together: one follower keeps at a time (`KEEPING`), so one turn.

## Done when

- `cargo test -p orchester-app --test orchestrator retry` covers the rows' states and **Try again**.
- An independent design verification of each state against its frame (`AGENTS.md` rule 15).
- `tools/check.ps1` passes.
