# Shared configuration on the desktop (ORC-389, N7b)

Status: draft for the person's review (2026-10-29). Design only; the screens are built after approval
(`docs/architecture/server-new-app-plan.md`, N7b).

In a shared project the five libraries (Pipelines, Profiles, Instructions, Skills, Tools) are the team's
(`docs/architecture/server.md` rows 30, 35, 36, 52). Only admins, the project's owner or the team server's admin,
draft, publish, create, import, rename and archive them (row 72, ORC-380, core `sharing::may_change_configuration`);
members read them and run with them. What a version needs that belongs to a person (a connection, an MCP server, a
secret) is bound on each computer, MCP servers and secrets only by the person.

## Where it is

File: the desktop design `crates/desktop-app/design/orchester.pen`, board **Shared configuration · redesign**
`UC5xk` (x 16760, y 9948), directly under *Agents · redesign*. Frames are copies of the Agents and Project settings
redesign frames (`acOlR` 01b Profile, `B94gsw` 01 Profiles, `TnUGj` 04b Sharing · shared, `JeXKt` 04c Share) with
their pane replaced; the modals follow `05d · Delete connection` (`D3c8ht`). Controls are instances of `Button/*`
(`iBDdP`, `O1AstI`, `YaUJX`), `Select/Default` (`aIu9u`), `Menu` (`Qa7Oo`), `Badge/*` (`NAQaS`, `V0dDbe`, `ysz94`),
`Avatar/Person` (`CpRbX`), `Kbd` (`WYYh4`) and the brand marks. Sample data: Acme team on
`orchester.acme.dev/kontur`; admins dana-k (owner) and olha (server admin); member ivan. Exports at scale 1:
`.tmp/design/ORC-389/<node id>.png`.

| Row | Node | Frames (dark) | Light copies |
| --- | --- | --- | --- |
| Head | `lKTrx` | title and lead | |
| Row 1 · Libraries | `WBcZ9` | `cHvkL` 01 Profiles · shared · admin (publishers, her draft, a version waiting, missing here, published here not sent), `P8tXS` 01b Profiles · shared · member (read-only, the admins named, no New or Import) | `flVbZ` 01L, `pAB0Z` 01bL |
| Row 2 · Editor header · admin | `uKLQh` | `Sflw7` 02 draft on v4 with *Publish for the team*, `iLwm9` 02b *v? · published here, not sent*, `JZFlA` 02c conflict with v5 (Compare, Open my draft on v5, Publish mine as v6), `FuOXA` 02d *v5 widens what agents may do here* with *Use v5 here* | `UrSw2` 02L, `Q1Wtpk` 02bL, `pLCwC` 02cL, `N7SLf` 02dL |
| Row 3 · Member | `KD90G` | `pjEjf` 03 Profile · member · read-only | `uUzBk` 03L |
| Row 4 · Missing here, bindings | `uOk9L` | `w0Evp` 04 Profile · missing on this computer, `e41po` 04b Sharing › On this computer, `rpQ7s` 04c binding picker for a connection, `z0iFy` 04d bind an MCP server (person-only modal), `LRXYn` 04e bind a secret (person-only modal) | `Jr2O1` 04L, `flgJ3` 04bL, `YKZL4` 04cL, `qk8Gb` 04dL, `ziM4s` 04eL |
| Row 5 · What sharing sends | `HUcP1` | `bqc8X` 05 Share with the updated *What sharing sends* | `cLOwD` 05L |
| Row 6, Row 7 · Light | `E3JoE`, `XqAs9` | the light copies above (`theme: light` on each frame) | |
| Row 9 · Runs here use (ORC-401) | `LbIBH` | `sakAV` 06 Profile · member · runs here use (Default and Work, Work chosen; page head *runs here: Claude Code · Work*), `p779C` 06b the account picker open (Default ✓, Work, *Add an account…*), `FO7Cq` 06c no Claude Code account (*Missing on this computer*, *Add an account…*), `q36bo` 06d Profile · admin · draft with *Runs here use* (publishing never sends it), `C8JAJ` 07 Sharing › On this computer with *Change* on rows bound by name and by role, `zE2cB` 07b *Change* picker on a row bound by name (Default, Personal, Work ✓, *Add an account…*), `A3rnh8` 07c On this computer without a Claude Code account | Row 10 `s10Hk`: `wPYC7` 06L, `Rg3Rj` 06bL, `z9R03e` 06cL, `zaIRx` 06dL, `EOV1X` 07L, `X9Nk2` 07bL, `lSMyI` 07cL |
| Row 8 · Notes | `x0m4ys` | `hauf7` what the design decides (with `e9C4O` *Runs here*, ORC-401), `O8q7H8` open choices | |

## Decisions (proposed)

1. **Who edits.** Admins see New, Import, the draft bar and Publish. A member sees the same libraries and editors
   read-only: no header actions, no draft bar, values as plain text, and one calm line with a lock: *Only admins
   change the team's configuration: dana-k (owner) and olha (server admin).* (01b, 03).
2. **Library states.** Every row shows its publisher (avatar, name, when) and one state: *vN*, *vN · your draft*,
   *vN waits for you*, *Missing here*, *Published here, not sent*. The same in all five libraries; drawn in Profiles.
3. **Publish for the team.** In a shared project *Publish vN* becomes *Publish for the team*; the bar says the draft
   stays on this computer and that publishing makes vN for the whole team (02). The Orchestrator keeps proposing a
   card for shared resources (row 35).
4. **Offline publish** (row 36): *v? · published here, not sent · 16:52*, the person's own new runs use it now,
   *Try again* (02b).
5. **Conflict** (`409 version_conflict`): *olha published v5 while you edited v4*, the draft kept, *Compare with v5*,
   *Open my draft on v5*, *Publish mine as v6*. Never merged (02c).
6. **Widens** (row 35): the bar lists what v5 adds (an MCP server with its binding here, network); runs keep v4;
   *Use v5 here* with the line *Only you can press this: not the Orchestrator, a tool or automation.* Shown to admins
   and members alike, since it is per computer (02d).
7. **Missing on this computer:** the resource says which reference is missing and that its pipelines can't run here;
   *Choose on this computer…* opens Sharing › On this computer (04).
8. **On this computer** (a section of Project settings › Sharing, `screens/project_settings/sharing/bindings.rs`):
   references grouped by kind (Connections, MCP servers, Secrets), each with who needs it and its state: bound by
   name, bound *by role* (badge), *Waits for you* with *Confirm…* (an MCP server found by name), missing with a picker
   (04b). The connection picker lists only connections of the reference's role; others show *another role* as
   unavailable (04c).
9. **Person-only confirmation** for an MCP server or a secret is a modal (confirmations are modals, decided
   2026-10-28): the choice, what it lets agents or tools do, that nothing about it is sent, and *Only you can confirm
   this; the Orchestrator and tools can't.* (04d, 04e).
10. **What sharing sends** gains *Team configuration* and *Named, not sent* (marked New); *Stays here* lists drafts,
    accounts, MCP server settings, secret values and environment; the note drops branch names, which shared progress
    now sends (05).
11. **Runs here use** (ORC-401, decided by the person 2026-10-29). In a shared project every person chooses for
    themselves which of their own accounts the team's agents run with on their computer. The profile stays the team's
    (admins change it); the connection and account it runs with here are the person's. This follows
    `agent-accounts.md` (*Accounts are app-wide and belong to the person; a project connection chooses one*) and
    `server.md` rows 30 and 52 (connections and CLI accounts are per person and local).
    - **Profile, member** (06): *Connection & model* shows the team's choice read-only (*Agent CLI* Claude Code,
      *Model*, *Reasoning effort*, each *The team's choice*) and one editable row, *Runs here use* with the badge
      *Yours*: a select of the person's own connections for that CLI with their accounts (*Claude Code · Work*), the
      hint naming the CLI version and plan. The note under the block: *Runs here use stays on this computer and is
      never sent to the team.* It replaces 03's read-only *Project connection* (the publisher's account). The page
      head reads *v4 · published by dana-k · Oct 27 · runs here: Claude Code · Work*.
    - **The picker** (06b, 07b) lists the person's connections of that role with their accounts (*Claude Code ·
      Default*, *· Work* with its plan, *· Personal*), the current one checked, then a separator and *Add an
      account…*, which opens Settings › Accounts. The profile's select and Sharing's *Change* open the same picker.
    - **On this computer** (07): every connection reference can be changed, not only a missing one. A row bound by
      name or by role keeps its status and *by name* / *by role*, and gains a secondary *Change*; after a choice the
      row says *chosen here*. The lead adds *and can change any of it*.
    - **No account of that CLI** (06c, 07c): *Missing on this computer* in the page head; the row's hint in `danger`
      (*Missing on this computer: no Claude Code account here · Builder can't run here*) and *Add an account…*
      (secondary) instead of a select. On this computer counts it among *need you* (*7 references · 4 need you*).
    - **Admins** (06d): the same per-computer row for their own runs, next to the team's editable CLI and model. The
      draft bar says *publishing makes v5 for the whole team, without Runs here use*; the note says publishing never
      sends it and each member picks their own.

## Open choices for the person

- **A.** On this computer as a section of the Sharing tab (proposed), or its own Project settings tab?
- **B.** *Publish for the team* without a confirmation (proposed), or a confirmation modal naming the team?
- **C.** The member line names the project's owner and the server's admins even when an admin isn't a member of the
  project (proposed).
- **D.** (ORC-401) *Runs here use* on the profile and *Change* on On this computer set the same per-computer binding
  of the profile's connection reference (proposed), so changing it in one place shows in the other.

## Notes for the implementation

- No shared component or variable was changed or added. The draft, conflict, widens and missing bars and the
  *What sharing sends* panel are frames from theme roles (`surface_raised`, `border`, `border_strong`, `hover`,
  `warning`, `danger`, `info`); the modal follows `05d` (`scrim`, `surface_raised`, `border_strong`, `shadow`).
- Gaps and paddings of 4, 8, 12, 16, 24 and 32 use `spacing_*`; the others (2, 6, 10, 11, 14, 28) have no variable.
  Control heights are plain 28 (`control_height_md`): Pencil keeps no variable on a height.
- The editor panes scroll: in 02c, 02d and 04 the last rows (*Reasoning effort*, the note) are hidden so the frame
  shows no clipped content; the built page keeps them below the fold. In 06d *Reasoning effort* is hidden the same
  way; in 06–06d the pane's gap is `spacing_lg` instead of `spacing_xl` so the block fits.
- ORC-401 controls: *Runs here use* is `Select/Default` (`aIu9u`), open `Select/Open` (`QyMAn`); the picker is `Menu`
  (`Qa7Oo`, one item replaced to carry the plan and the check); *Change* and *Add an account…* are
  `Button/Secondary/Default` (`O1AstI`); *Yours* and *by role* are `Badge/Neutral` (`NAQaS`). The row's tint is
  `hover`. Exports: `.tmp/design/ORC-401/<node id>.png`.
- Copy: `sharing.config.*` (library states, the bars, conflict, widens, missing, the read-only line),
  `sharing.bindings.*` (On this computer, the picker, the two confirmations, *Change*, *chosen here*),
  `sharing.sends.*` (updated), `sharing.runs_here.*` (ORC-401: *Runs here use*, *Yours*, *The team's choice*, the
  note, *Add an account…*, the missing line, the admin's draft line).
