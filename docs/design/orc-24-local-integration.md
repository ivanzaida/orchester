# ORC-24: local checkout integration

The person approved this feature design on 2026-10-11 and confirmed saving the native Pencil document with Ctrl+S.
The source is `data/orchester2.pen`, section `zMOBO`, **ORC-24 · Local integration · editable feature design**.
Read the encrypted document only through Pencil. The earlier browser prototype is not the design source.

The feature lets a person finish local integration inside Orchester when their main checkout has local changes the
merge would overwrite or the task merge needs conflict resolution. Confirmations are inline; new and editing surfaces
are pages. Fixture names, paths, commits and task data in Pencil are examples, never defaults or shipped project data.

**Entry from the Summary (2026-10-18).** *Merge into main* on the Ready card merges the run directly, as the domain
tool `merge_run` does (`orchester_core::integration::merge`): in a temporary worktree, brought into the checkout as a
fast-forward that leaves uncommitted and untracked files it does not reach exactly as they are. It does not open
Local integration first. The card offers *Local integration* (an action in *Not merged into main*) only where it
carries on: the merge would overwrite files with uncommitted changes, untracked or ignored files; the run conflicts
with the branch; or this run's local integration holds work it must finish, put back or clean up (preserved originals, also
originals kept after a cancellation, a commit, a sync, the run's worktree taken over, a resolution, unsaved editor
decisions, or a Git process that has not finished). A checkout on another
branch or with a git operation in progress, and another task's local integration holding the checkout, are refused
with the reason and no Local integration action. A review that only read the checkout, also one paused or stopped
since, or a cancellation whose kept originals were removed, holds nothing: it does not stand in the way of the
direct merge, and once the run is merged it reads as complete (only originals still kept wait to be put back). While
Local integration asks a person for a decision in the project, or works in the checkout, *Merge into main* waits
and says so. A press that comes from an MCP client through the window goes through `merge_run`'s policy, which
refuses and redacts what that tool would.

**Merged outside Orchester (2026-10-18).** A run whose whole work Git finds in the checkout's branch (complete
ancestry: a merge by hand, or a pull request pulled back) is merged, in every run view: the Summary and its *Runs of
this task*, the task list, Needs you, the Orchestrator chat's cards and the domain tools (`list_runs`, `get_run`'s
`merge`, `get_task`). `merge_run` answers `already_merged` for it instead of asking for a person. No Orchester merge
event is invented for it. Only proven containment of the whole branch counts: part of it, equivalent patches or an
equal tree still wait for the person. Its local integration record then settles as follows, never a dead end:

- A record that holds nothing (a review, also one paused or stopped since, or a cancellation whose kept originals
  were removed) completes by itself when the tab or the Summary reads the evidence, and stops reserving the
  checkout; its recovery record says the merge was made without Orchester and at which commit. Nothing changes while
  another local integration works or a Git process of this one has not finished; a later reading settles it.
- A record that holds the person's work keeps its card beside *Merged*. Where it stopped between steps (a review, a
  prepared or checked checkout, or a cancellation, also one paused or stopped for reconciliation since), with no
  resolution, unsaved editor decisions or task abort in flight and any preserved originals verified, *Record
  this merge and finish* records the merge Git proves as the record's own, as a merge Orchester recorded would be:
  preserved originals are put back next (*Restore*) and removed as usual, and a record with nothing else to finish
  completes. The merge stays one made without Orchester: reconciling it later never writes an Orchester merge record.
  Restoring needs the checkout still at that commit; a pull in between makes *Restore* refuse as stale.
- Mid-step records and partial or equivalent evidence offer what works with the checkout as it is: a task merge in
  conflict is aborted, a cancellation keeps its own actions, a record still at the commit it reviewed (and, with
  originals preserved, the commit they were taken from) is cancelled, and one whose checkout moved since is
  reconciled first, which reviews the moved checkout so a record without preserved originals can then be cancelled.
  Preserved originals taken from another commit are never written over a moved checkout: they wait for the person
  (complete evidence lets them record the merge and restore onto it).

Nothing is merged, synced or resolved again in any of these.

The page shows these states as drawn in `zMOBO`: `itt7z` (21, the record held nothing: the *Merged* card once, on the
evidence side, and the recovery record's Merge row "Without Orchester · <commit>"; no step card), `pdT4W` (22,
preserved originals: *Merged*, then *Restore preserved local changes · next*; the step card with *Record this merge and
finish*) and `UWLNj` (23, partial evidence: the reconciliation warning; the step card with *Reconcile actual Git
state*). The review's file list and phase note give way to the evidence card in these states. A record completed this
way says the merge was made without Orchester instead of claiming a recorded merge and restoration. On the Summary
(`fvCVT`, board 9) the run reads merged, its row "Merged into main without Orchester"; an Orchestrator merge proposal
for such a run (`RWBN8`) reads "Run #N of KEY is in main already", no longer waits for the person and offers only
*Open changes*.

## Reviewed native states

| Node | State | Logical size |
|---|---|---|
| `YxF6B` | Local staged, unstaged and untracked files and diff | 1440 × 1080 |
| `kexy4` | Selected commit review; excluded edits retained | 1440 × 1080 |
| `UgHeC` | Named, verified temporary preservation | 1440 × 1080 |
| `g4KepC` | Partial commit recorded; remaining blockers | 1440 × 1080 |
| `g6ehAu` | Sync and fresh Check; safe stop request | 1440 × 1080 |
| `OoP6s` | Separate approval of the exact checked result | 1440 × 1080 |
| `loXXi` | Task text conflict selector, versions and editable result | 1440 × 1300 |
| `i0tWhP` | Binary, rename and delete decisions | 1440 × 1300 |
| `o6ktb4` | Already merged; local restore conflicts and staging choice | 1440 × 1440 |
| `AdLRF` | Recorded merge and verified restoration; recovery retained | 1440 × 1080 |
| `TnpfP` | Restart reconciliation before safe continuation | 1440 × 1080 |
| `MrnFV` | Commit hook failed; checkout and index intact | 1440 × 1080 |
| `i6Cx4` | External checkout change or foreign Git operation | 1440 × 1080 |
| `MBZu9` | Unsaved result: save and pause, keep editing or discard draft | 1440 × 1300 |
| `TfJls` | Scoped task merge abort confirmation | 1440 × 1080 |
| `It5Lh` | Preservation removal after verified restoration | 1440 × 1080 |
| `s4gj9` | Narrow local selection and diff | 960 × 1400 |
| `Ol3sv` | Narrow task text resolver with stacked versions | 960 × 1600 |
| `CJjYl` | Narrow restoration and editable binary destination | 960 × 1900 |
| `M0iuzM` | Narrow exact-result approval | 960 × 1200 |

These are extended page compositions. They do not imply that all controls fit a 900-pixel-high window: long resolver
and restoration content needs app-local scrolling. Keep the main review and approval actions near the top.

## Behavior that implementation must preserve

1. Review real staged, unstaged and untracked content, with explicit ignored-file boundaries. Selected-index commit
   review compares HEAD with the selected index. Preserve excluded staged and working-copy changes exactly.
2. Give temporary preservation a name and verified identity, including tracked/untracked content and staging.
   Keep originals until restoration is verified. Do not blindly pop a stash or stage the entire checkout.
3. Sync first, run a real fresh Check, then request separate approval of the exact checked merge result. Relevant
   checkout or result changes invalidate that approval. Waiting for approval must not hold the queue.
4. Resolve task conflicts in the app: multiple-file selection, source versions, editable text results, deliberate
   binary/rename/delete choices and editable keep-both destinations. Save and resolved gates reflect actual state.
5. Distinguish a task merge conflict from restoration of local edits after an already recorded merge. Restoration
   offers explicit Stage or Leave unstaged. A restore failure or restart must never repeat a successful merge.
6. Persist operation ownership and recovery state. Reconcile actual Git state after restart before choosing a safe
   continuation. Refuse unrelated or foreign Git operations. Cancellation and abort affect only the owned operation.
7. Commit hook failures preserve content/index and return to review without reporting a successful commit. Unsaved
   editor decisions, scoped abort and irreversible preservation removal remain explicit person decisions.
8. Show progress and success only for work that occurred. Keep retained recovery records until verified cleanup.
   Secret values must not appear in previews, logs, prompts, copied content or MCP output.

Domain rules stay independent of the desktop. Use the existing core/workspace, execution and store boundaries;
provide equivalent Windows and macOS behavior or an explicit visible unsupported state. UI uses the existing native
theme roles and i18n. Independent source review, meaningful filtered tests, `tools/check.ps1`, and independent live
app/Pencil comparison are required. Protected person-only replies are answered by the person.

## Design review and remaining runtime evidence

Independent native design review found no blocking or major issue in all twenty final frames after corrections.
It did not verify working Git behavior, durable journal atomicity, live scrolling/focus, accessibility, large files,
light theme, macOS chrome or every dropdown/error variant. Queue/Summary entry wiring and sensitive-preview behavior
need implementation evidence. Align illustrative filenames across states when preparing QA fixtures.

The design-only exports, frame index and independent review are retained locally under
`.tmp/orchestrator/integration-design/pencil/`. Their earlier notes said the disk save and human approval were pending;
the person's subsequent approval and Ctrl+S confirmation supersede those persistence limits. Neither drawn controls
nor design approval constitute runtime verification or authorization to bypass protected input guards.
