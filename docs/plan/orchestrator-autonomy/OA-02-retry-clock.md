# OA-02 · The event's line says what happens to a turn that failed

- **Status:** built (2026-11-14); design verification pending (a separate agent verifies)
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

## As built (2026-11-14)

Frame: `Parts · An event whose turn failed or is held` `tkjdw` a–c on the board `Orchestrator autonomy · frames to
approve`.

- **What the conversation says is read with it** (`orchester_workspace::orchestrator::event_turns`, a field of
  `ChatView`): per event, *tried again* (at what time, which try of how many, how the last turn failed), *trying*
  (the try that thinks, from the second on), *left to the person* (after how many tries) or *held* (OA-03). It says
  what the follower will do, by the same rule: a failed turn is tried again only while its wait stands, the switch is
  on and the Orchestrator has something to run on; otherwise the row shows how its last turn ended, as before. The
  earlier tries of a conversation's waits are read with `ProjectDb::chat_wait_tries`.
- **The row** is `screens::orchestrator::wait::turn_state` (a component under the event's row, after its answer): a
  `rotate-cw` and *Couldn't answer — trying again at 14:05 · 2 of 4* with why; a `circle-alert` in `warning` and
  *Left to you after 4 tries* with why and **Try again**. The try that thinks is the working line's:
  *Orchestrator is thinking · 2 of 4* (`ending.rs`). The divider a failed turn of an event ended with is replaced by
  the row while there is one.
- **Try again** is `orchester_workspace::orchestrator::turns::try_again_in`: one more turn for that wait now, also
  after the last try by itself, refused while the Orchestrator has nothing to run on, once the wait no longer stands,
  and while another turn of the conversation thinks. The app gives it under the follower's lock and asks the runner
  for it as the follower does (`project::waits::try_again`), as a task of its own. Only a person presses it:
  synthetic input is refused to the client that sent it outside a QA copy (`mcp::qa_person`), as *Stop watching* is.
  A try that started nothing says why in the reason's place.
- **Differs from the frame:** the event's own row keeps its status chip (*Waits for you*, *Orchestrator is reading*),
  which the frame's row does not draw; **Try again** is the kit's secondary button (its label 12/500, the frame's
  12.5/500). A time on another day says its day too (*Nov 17 16:40*).

Tests: `orchester-workspace --test orchestrator_held held` (the states, the schedule's tries, *Try again* and its
refusal); `orchester-app --test orchestrator leading` (the rows, the refusal of synthetic input, a try that started
nothing); `--test seed orchestrator` (the QA scenario `orchestrator-leading`). Not run: a try in a running app.