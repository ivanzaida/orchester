# Decision reviewer: jev, Laya, or Laya on this computer

Designed 2026-10-31 after a test drive of TypeSafe's jev on this project's recorded requests (`E:\jev-testdrive`,
`jev-vs-haiku.md`): on 9,803 requests Claude Haiku 4.5 had reviewed, jev agreed 97.9% of the time, answered in a median
182 ms against 10.2 s, and cost about $0.88 against about $80 at API prices. The design source is
`crates/desktop-app/design/orchester.pen`, board `Decision reviewer · redesign` (`a6B6v`), read and changed only through
the pencil MCP server. Every name, path, number and time in the frames is illustrative (Kontur, Builder, jev · TypeSafe,
the test's figures), never shipped data. Exports at scale 1: `.tmp/design/decision-reviewer/export/<node id>.png`.

## Where it is

| Frame | Node | What it shows |
|---|---|---|
| 01 · Add a decision model · hosted | `XMNUP` | Connections › Add › Decision model with jev on api.typesafe.ai: server cards, name, base URL, key, model, the warning callout, person-only save |
| 01b · Add a decision model · on this computer | `z3G09h` | The same form for laya-serve at 127.0.0.1: no key, the green callout, Laya's calibration note |
| 02 · Decision model connection | `GM4yE` | The connection page: server, where requests go, models, the test on this project's requests, used by |
| 03 · Runs · Approvals · a decision model reviews | `noZlm` | The reviewer row with connection, model and threshold; the Estimates hint |
| 04 · Reviewer row · states | `Q05Ish` | a the connection menu, b the question in place, c Laya on this computer, d editing the threshold, e the server does not answer |
| 05 · Command approval · a decision model reviewed | `JFEPS` | a a risk named, b nothing stood out, c the model did not answer |
| 06 · Answered without you · a decision model | `xJ4Rf` | Quiet lines with the probability; an opened line with the verdict and the five risks |
| 07 · Decision model connection · states | `FJRFV` | checking, key refused, nothing answers on this computer, not the System One API, model no longer listed |
| Notes · What the design decides | `KEcBv` | The decisions below |

## Decisions (proposed)

- **A connection kind of its own.** A decision model is added from Project settings › Connections › Add › Decision
  model. It speaks the System One API: `POST /v1/systemone` with `model`, `state` and typed `questions`, and
  `GET /v1/models`. Servers known today: TypeSafe (`https://api.typesafe.ai/v1`, jev), Laya Studio
  (`https://api.laya.studio/v1`), and laya-serve on this computer (Laya, Apache-2.0, `pip install 'laya[serve]'`). The
  base URL is a real field, not a knob for later: three servers speak the same API. It reviews only; no agent profile can
  choose it.
- **Fields.** Name, base URL, API key (a project secret, optional: laya-serve takes none), and the model from the list
  `GET /v1/models` gives. Check calls only that endpoint, so it sends no request data.
- **Where requests go.** A server at `127.0.0.1` or `localhost` keeps requests on the computer. Any other host shows the
  warning callout, a *Requests go to* row on the connection page and, when it is chosen as the reviewer, the question in
  place. Saving such a connection and choosing it as the reviewer are a person's alone: over MCP both are refused as
  `person_only`. A local one may be set up by automation.
- **What it is asked.** One request per review. `state` holds the same three parts as the language reviewer's prompt
  (`crates/core/src/run/review/prompt.rs`): the step's facts, the task, and the agent's data, which is untrusted and
  kept apart. Questions: `verdict` (`choice`: approve or ask, the rules of `REVIEW_INSTRUCTIONS` in its instructions)
  and five `noul` risks: writes outside the worktree, reaches the network, reads a credential, discards work, does not
  serve the task. For Autopilot's roots question the verdict asks `ROOTS_QUESTION` instead.
- **Threshold.** Per reviewer, 0.80 by default, edited in the reviewer row in place of the effort. At or above it the
  request runs and leaves a quiet line with the probability; below it the approval card comes with the model, the
  probability and the highest risk at 0.5 or more. 0.80 is where the test drive let no risky request through (29 of the
  107 Haiku had left to the person, none of them destructive).
- **Test on this project's requests.** The connection page runs the model on the requests the project's reviewer judged
  before and shows per threshold how many it would approve, how many come to the person, and how many the earlier
  reviewer left to the person it would approve. It sends those requests to the server, says so, and runs only when the
  person presses *Test*. Laya is known to be over-confident until calibrated, so its threshold should come from this
  test.
- **Fails closed.** A timeout (2 s), a refused key, a server that does not answer, an answer that cannot be read, or a
  model that is no longer listed sends the request to the person, with the reason on the card and in the reviewer row.
  Nothing falls back to another reviewer.
- **Estimates** need a written reason, so with a decision reviewer they use a language model connection of the project.

## Shared projects: the team's reviewer

Designed 2026-10-31. Web: `H:/projects/orchester-cloud/web/design/orchester-web.pen`, board
`Team server · Decision reviewer` (`ViIs2`): R1 `d1EaS` (Agents › Reviewer, admin), R2 `v1V24P` (member), R3 `FJ94M`
(edit and publish), R4 `N4Tfa` (states). Desktop: 08 `tgsJq` on the board above.

- **A shared resource with versions.** The team's reviewer is a resource of the shared configuration, like a profile:
  Agents › Reviewer on the team server's web app, versions numbered by the server, published only by the project's
  owner or a server admin, read by every member. Without one (R4 a) each member keeps the reviewer they chose in their
  Orchester.
- **What it holds.** The kind (language or decision model), the connection as a reference (kind, name, role; each
  computer binds it, the key never leaves the computer; **changed 2026-11-07, built as ORC-887:** a language model names
  a CLI kind that each computer picks a connection for, like a team profile, and a decision model names no connection:
  its kind implies a decision connection, which each computer picks too, so no field is added for it), the model from
  those members' computers reported for it, the effort for a language model, the lowest threshold for a decision model, and the servers reviewed requests may go to:
  *Members' computers only* (127.0.0.1 or localhost) or *These servers* (a list of hosts).
- **Reviews still run on members' computers.** The server stores the setting and its versions only: never a request, a
  verdict, a probability or a key. Nothing about reviews is added to what shared run progress reports.
- **Widening waits for each person.** A version that lets requests go to a server the previous one did not (R3) waits on
  each desktop: the reviewer row asks in place (08 b), and that computer keeps the earlier version until its person
  accepts. Raising the threshold, *Members' computers only*, or removing a server reaches members without a question.
- **On the desktop** (08). The row shows *Set by the team · vN* with the connection and model locked; the member may
  raise the threshold above the team's, never lower it. An unbound reference sends every request to the person until it
  has a connection picked on this computer (08 c; ORC-887: *Needs you here* until a connection is picked); binding to a
  server the team does not list is refused (08 d; `server_not_allowed`). **Built so far (ORC-887):** the pick, its
  refusals and the reviewer's readiness exist. Binding the team's reviewer into a run is not built: a run still gets its
  reviewer from the project's own setting, so a team reviewer's pick changes no review yet (`docs/plan.md`).
- **Contract.** The resource kind, its fields and the widening rule go into `orchester-api` and `docs/cloud-api.md` here
  first; the server and web halves are the Orchester Platform project's.

## Not in this design

- A cascade: a decision model first, a language model for the uncertain middle.
- An OpenAI-compatible chat reviewer (Ollama, OpenRouter, vLLM).

## Notes for the implementation

- The boundary is `RequestReviewer` (`crates/core/src/run/review/mod.rs`); a new `ReviewerKind` beside `ClaudeCode`,
  `Codex` and `AnthropicApi` (`choice.rs`), whose `reviewer_kind` today refuses everything but agent connections.
- `Review::Approved` and `Review::Asked` carry a reason string; a decision verdict needs the probability, the threshold
  and the named risk as data, not text, so the card and the quiet line can show them.
- `reviewer_usage` records the model and input tokens; System One answers have no output tokens.
- The test drive's harness (`E:\jev-testdrive\jev.mjs`, `.tmp/jev/export.py`) is the reference for the test's figures.
