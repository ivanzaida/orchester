# Frames the person draws

The person decided on 2026-11-13 to draw these themselves. Each group names the task that waits for it, where it
belongs in `crates/desktop-app/design/orchester.pen`, the frames and their states, and the words proposed for them
(the words are suggestions; the frames decide). Everything is drawn with the boards' own components and theme
variables, dark and light where the board has both. Fixture data as on the boards (Kontur, KON-41, Webhooks v2).

Order of use: **A** unblocks the first milestone; **B** is needed before a person can turn *Lead to the end* on in
the window; **C** and **E** follow; **D** and **F** last.

## A · An event whose turn failed or is held (OA-02, OA-03)

Board `Orchestrator WIP` `mPw5l`, beside the tool request's states (`ICzi3`): the row of an event in the
conversation, in these states.

| State | Row reads |
| --- | --- |
| Tried again later | *Couldn't answer — trying again at 14:05 · 2 of 4*, in the neutral tone, with why on hover or under it (*the answer took longer than 10 minutes*) |
| Trying now | the existing *thinking* row, with *2 of 4* |
| Left to the person | *Left to you after 4 tries*, warning tone, with **Try again** (ghost button) and the event's own link |
| Held by the budget | *Held until 00:00 — today's budget is used*, neutral, no action |
| Held by the plan limit | *Held until 16:40 — the account's plan limit is reached*, neutral, no action |
| Held, reset unknown | *Held — the account's plan limit is reached · trying again at 15:00* |

And the conversation's header while its turns are held: one quiet line or chip, *Orchestrator is held until 00:00*,
that does not push the *Leads* chips.

## B · Choosing *Lead to the end* (OA-10)

Board `Orchestrator watch · redesign` `QOPcg`.

1. **The pickers** (copies of 02 `c8pALl` and 03 `mQdxa`): a choice between *Follow* and *Lead to the end* in the
   picker, reachable by keyboard, with one line under it that says what leading means (*Starts its tasks, runs failed
   ones again and leaves you what it cannot do*). Where it sits relative to the key caps in the footer is the
   frame's to decide.
2. **The line and the chip** (a copy of 04 `OKtX2`): the app's line *Leading Webhooks v2 to the end · 12 tasks — it
   starts them, merges them and parks what needs you*, and a header chip that tells a leading subscription from a
   following one (an icon or a word, not colour alone).
3. **/watching and the *+N* list** (copies of 06 `Ff7AD` and `Jl9jP`): under a leading subscription its ceilings with
   what is used (*Runs of a task 3 · Returns per run 4 · Raises 1 by 50 % · Budget 12M of 40M tokens*), and the way
   to change the mode or a ceiling there.
4. **Editing ceilings**: the small form or popover that opens from 3, with its bounds' errors (runs 1–5, returns
   0–10, raises 0–3, percent up to 100, budget empty or a number).
5. **States**: a ceiling reached on a task (*KON-41 · run again 3 of 3 — parked for you*), the budget used (*Budget
   used — nothing new starts; what runs finishes*).
6. **Project settings › Orchestrator › *Watching*** (a copy of 07 `DE2Pj`): rows for the defaults a new leading
   subscription takes (the four ceilings).
7. Light theme of 1–3.

## C · Before you leave (OA-19)

Board `Orchestrator watch · redesign` `QOPcg`.

1. A card under the leading line: **Before you leave** with a line per finding, each with its place to fix it as a
   link, in the warning tone; for example
   - *Let the Orchestrator drive is off* → Project settings › Orchestrator
   - *KON-47 has no pipeline* → the task
   - *Orchester change has no Deliver step — its merges are not checked after merging* → the pipeline
   - *Your checkout has local changes — a merge that touches them will wait for you* → Local integration
   - *Pencil design is turned off — Verify cannot read the design* → Connections
   - *The computer may go to sleep* → Settings
   - *Closing the window stops the work* (no link)
   - *Not checked: whether Pencil answers*
2. The all-clear state: one quiet line, *Ready to leave*.
3. The same list inside the `/watching` answer.

## D · A local module's own branch, acceptance and delivery (OA-17, OA-20)

The module form and the module's Summary (the boards that hold them today).

1. **The module form**: *Its own branch* (a switch, off by default) with what it means in one line (*Tasks merge into
   `module/webhooks-v2`, not into your checkout*); under it, only while on: *Acceptance pipeline* (a pipeline picker,
   may be empty) and *Deliver when accepted* (a switch, disabled with its reason while no acceptance pipeline is
   chosen).
2. **The module's Summary** with its branch: the branch's name, how many tasks are merged into it, how far it is
   ahead of and behind its base branch, and the primary action **Deliver** (or the word the frame chooses; the team
   server says *delivery*).
3. **States of that row**: nothing to deliver yet; the base moved (*3 commits behind main — taken in before
   delivery*); acceptance running (*Accepting · run #31*); accepted; acceptance found problems (*2 findings became
   tasks*); delivering; delivered (*Delivered to main · a3f14fb*); a delivery the checkout refused (*Waits for you on
   Local integration*); conflicts with the base.
4. The acceptance task in the task list and on its page, told apart from the module's own tasks.

## E · While you were away (OA-23)

The *Needs you* page, the task page (`Task page · redesign`) and the leading conversation.

1. ***Needs you* › Parked for you**: a row per parked task — the task, what is needed (*An approval*, *A decision*,
   *A sign-in*, *A tool did not answer*, *Tried 3 times*), why the Orchestrator could not, what to do in its words,
   the action that settles it (the request's card, the question, **Give it back to the Orchestrator**) and the tasks
   it holds up.
2. ***Needs you* › Decided without you**: the open decisions, newest first, grouped by module — what was done, the
   one-sentence reason, the task and the turn as links, **Seen** and **Overrule…**; **Mark all seen** at the head;
   the count in the page's tab or header.
3. **Overrule**: the conversation opens with the decision quoted in the composer.
4. **The task page of a parked task**: the header's status line (*Parked for you — an approval is needed*), the
   Summary's wait row with what to do and **Give it back**, properties *Led in*; in the warning tone, like `rlnEZ` f.
5. **The leading conversation**: the row *KON-41 parked for you — …* and the event *Webhooks v2 is stuck — 2 tasks
   are parked and 3 wait on them* with its turn.
6. **States**: nothing parked; nothing to review; loading; cannot be read; light theme.

## F · Keep the computer awake (OA-08)

Settings › General (or where the frame puts it): one row, *Keep the computer awake while agents work*, a switch, with
one line under it (*Only while an agent works or the Orchestrator answers; the screen may still turn off*), and its
state on a platform where it cannot be held (*Not available on this system*).
