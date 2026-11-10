# PV2-17 · Readiness polish: one story in the chip, *Ready?* and the cards

- **Status:** in progress (2026-11-11; half b — items 14–16, 18, 19, 24–29, 31's pointer test, 33 — checked twice (review + design) and merged `238b0c2e6`; half a — items 1–13, 17, 20–23, 30–32 — built, round 2 (26's leftovers and a path-dependent override drop) running, then one check)
- **Kind:** UI
- **Depends on:** PV2-09, PV2-10 (merged)
- **Branch:** `claude/pv2-17b-step-flow` → `238b0c2e6`; `claude/pv2-17a-ready-save` (running)
- **Tokens:** half b 116.1M (out 239k) across 2 implementation rounds and 2 checks; half a: pending

## Goal

PV2-09's design verification (round 2) left UX findings that make the page tell different stories in different places.
Make the chip, the list line, *Ready?* and the step cards agree, and give every problem a way to its fix.

## Scope

1. **An undrawable pipeline** (*These steps can't be drawn as one line*): *Ready?* lists every problem the chip or the
   list line names (e.g. *Fetch has no agent profile*), not only the layout.
2. **One name per action**: *Ready?*'s *Add step →* and the panel's *Edit steps* do the same thing — use one word
   (the one PV2-06/PV2-10 settle on for adding a step in place; until then *Edit steps*).
3. ***Not published yet*** has an action: *Save →* (or the words of PV2-10's bar) that focuses / scrolls to the Save bar.
4. **Gone connections on the cards**: a step whose profile's connection is gone is amber on the flow with *Open profile*
   (as PV2-05's other amber cases), so a long flow shows where the problem is.
5. **Version problems under *Ready? · this draft***: rows that describe the published version, not the draft, are
   labelled as such or left out when the draft no longer has that step.
6. The unreachable `Runs::Profile` / `Runs::Tool` arms of `line_key` and `step_words` (`step_line.rs`, `ready.rs`) are
   removed or made unreachable by type, so no `{{count}}` placeholder can ever print.

## Done when

- Each point tested; an independent design verification of P1, P8 and the states named above (seeds
  `pipelines-flow`, `pipelines`, `pipelines-not-ready`).
- `tools/check.ps1` passes.

## Added from PV2-10's design verification and review (2026-11-11)

7. **Save after a draft that cannot publish**: when nothing changed and publishing would fail (steps not ready, no
   steps), Save is disabled and the hint says what to fix (*Fix the missing steps to publish*), instead of an enabled
   *Save makes v1* that fails again.
8. **The v1 hint on a ready draft**: *Save publishes it as v1* when every step is ready (today it always says *New task
   offers it once every step is ready*).
9. **The saved notice**: pinned above the bar like the refusal (today it ends the scroll area); its words follow
   readiness (*Saved as v4 · the next run uses it* only when the pipeline can run; otherwise *Saved as v4 · runs can
   start once every step is ready*).
10. **The draft-saved notice** names every missing step (or the count), not only the first.
11. **Closing the tab** with unsaved changes says *close* in its question, not *opening anything else*.
12. Review minors: `pipeline_hub/save.rs:224` discards a `Result` with `.ok()` without a comment (AGENTS rule 7 —
    must be fixed before the module goes to master); import `lurq::core::Ref` in `save.rs`; a refusal for an
    unchanged edit stays until the next Save (offer Cancel or clear it); after a Save in the *Drafts* tab the pipeline
    leaves the tab and the notice disappears with the panel — keep the panel on it (switch tab) or say where it went.

## Added from PV2-07's review and design verification (2026-11-11)

13. **Invalid text alone** (an Instructions or Budget the check refuses, with nothing else changed) does not count as
    unsaved: leaving the pipeline drops it silently and Cancel is not offered (`pipeline_hub/panel.rs` `mark_unsaved`).
    It counts as unsaved, or leaving says it is not kept.
14. **Stale invalid text** of a step that no longer exists stays in `Faulty` (`step/mod.rs` `live`, `fix_words`): once
    PV2-06's Remove step is in, a removed copy's bad text comes back on the next Duplicate (same `{key}-copy`). Drop
    entries for steps the edit no longer has.
15. **One Duplicate**: PV2-07's `step/head.rs` `copy_of` and PV2-06's `flow/reorder.rs` do the same rewiring with two
    copy keys (`pipeline_hub.step.copyName`, `pipeline_hub.copyName`): one helper, one key (whichever merges second, or
    here).
16. **Deliver's number**: the head says *step 06 of 6* and the strip's Deliver chip reads *07* (the design counts
    Deliver as 06): one numbering.
17. A disabled Save says why at the button (today only the bar's line on the invalid step does); the list row's
    *Unsaved changes* while Save is blocked by invalid text.
18. Tests the old retries test had: changing only the target keeps the retries; unfinished retries text is kept on a
    target change. A test that the *Agent* row says why a tool cannot run (`agentToolUnavailable`).
19. *Duplicate* puts the copy beside the original (parallel); decide whether a person expects it after the original.

## Added from PV2-11's review and design verification (2026-11-11)

20. After *Restore v2* nothing but the bar's *Unsaved changes* says the editor holds v2: a short *Restored v2 · not
    saved* in the head or the bar until Save or Cancel.
21. *Allow these changes?* after a Restore names the restore as the cause (today it lists the step only).
22. The read-only view says *Viewing v2* twice (head and notice): the notice keeps only *read-only*.
23. Tests: read-only cards are not Tab stops; the held *Versions* button's fill and border; the list reads again
    after a save or rename (if PV2-11's merge round did not add it).

## Added from PV2-08's review (2026-11-11)

24. **A stale model override**: a step that overrides the model and switches to a profile on another CLI keeps the
    override, and the panel no longer has a field to clear it (`workspace/src/pipelines/fields.rs` `Executor::apply`
    keeps it). Offer a way to drop it (e.g. on the picker's chosen row) or drop it when the CLI changes, and say which.
25. *Move up* / *Move down* on a step in a parallel group takes it out of the group: the menu's words say so (e.g.
    *Move up, out of the group*) or the move keeps the group, as decided.

## Added from PV2-08's check (2026-11-11)

26. A gone profile or tool: the field and the flow card say it the same way (today *This profile is no longer
    available* vs *Profile not published*), and the field says what to do (choose another, with the picker open).
27. A long model override cuts what the profile runs on (`Claude Code · Wo…`): keep the CLI and the connection whole,
    shorten the override, or show the override on a line of its own.
28. The step strip: pressing a chip that is partly hidden only scrolls it into view, so it takes two presses to open
    it. One press opens it (and scrolls it into view).

## Added from PV2-06's last check (2026-11-11)

29. A drop refused over a gap leaves nothing behind once released (the reason shows only during the drag): keep the
    refusal under the card's stage after the release until the next change, or say it some other way.
30. The *Allow* question's `pipelineStep` words still say "same wiring" (`workspace/assets/locales/en.json` ~1806):
    wiring is `StopsWaiting`'s now.
31. Tests: the Orchestrator's change-list line for `StopsWaiting` (`orchestrator.change.widens.pipelineStopsWaiting`);
    a refusal cleared by the pointer leaving its card (only the keys are tested).
32. Below about 1100 logical the hub's detail panel is crushed (strip, parallel cards and the Description row clip).
33. Which card the keys are on: the outline only; a card keeps its handle and ⋯ while it has the keys — decide whether
    that is cue enough.
