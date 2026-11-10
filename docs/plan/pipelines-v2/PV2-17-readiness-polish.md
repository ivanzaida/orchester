# PV2-17 · Readiness polish: one story in the chip, *Ready?* and the cards

- **Status:** todo
- **Kind:** UI
- **Depends on:** PV2-09, PV2-10 (merged)
- **Branch:** —
- **Tokens:** —

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
