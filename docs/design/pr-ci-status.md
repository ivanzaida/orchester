# A run's pull request CI and Fix CI (ORC-608)

Date: 2026-11-01. Status: design for review. Source: `crates/desktop-app/design/orchester.pen` (Pencil MCP only).
Every task, branch, check name, log line and time in the frames is illustrative.

When a run opens a pull request, the person wants to see its CI without going to GitHub, and to send failing checks back to
the agent in one click. Claude Code desktop does this with *Auto-fix* and *Merge when ready*. Here it is *Fix CI* (once, by
hand) and *Fix CI automatically* (a per-run switch with a limit of attempts). Merging stays on GitHub. *Merge when ready*
is not designed here.

## Frames

| Board | Row | Node | Shows |
| --- | --- | --- | --- |
| `Task page · redesign` `C3wC8` | Row 10 · Pull request CI · Summary (ORC-608) `A56TC` | `ysTLm` 10 · Summary · CI running | Header *CI running on #48*, primary *Open pull request*. Card `sSYu8` *Pull request #48* above Run: `Badge/Info` *Running*, five check rows (passed, running, queued) with *Details ↗*, the switch row (on, *3 attempts*, *none used*) |
| | | `mxt1S` 10b · Summary · CI failed · Fix CI | Header *CI failed on #48*, primary *Fix CI*. Card `J0SbA` (`danger` stroke): `Badge/Danger`, the two failing checks first, each with a log tail in `code_bg`, the passed ones folded into one line, *Fix CI* + *Open checks on GitHub* + hint, switch off |
| | | `YBx9T` 10c · Summary · CI passed | Header *CI passed on #48*. Card `DLnt0` (`success` stroke), five passed rows, *1 of 3 automatic fixes used on this run* |
| | | `uA3vm` 10d · Summary · gh not signed in | Header *Pull request #48 · CI not read*. Card `p7oVdN`: `Badge/Neutral` *Unknown*, `Callout/Warning` `U2Umy` *GitHub CLI isn't signed in* with the command chip *gh auth login* (`font_mono` on `code_bg`), *Check again* + *Open on GitHub* (`n6NeR`), switch row `WvQKS` off and disabled (switch `xddHn`, *up to*, select `IL2Ph` at 50 %) with *Needs gh to read checks* |
| | Row 11 · Pull request CI · Conversation and parts (ORC-608) `Wm7Nq` | `MK7l7` 11 · Conversation · CI failed | Event lines *Opened pull request #48* and *CI started*, then the CI card (`danger` stroke) with the failing checks, *Fix CI*, *Open checks on GitHub* and *Fix CI automatically is off for this run*. Composer: Builder *ended*, *Continue & send* |
| | | `X8xYZ` 11b · Conversation · Fix CI sent | The answered card folds into one `danger` event line. *Your message* holds the Fix CI text: lead, each failing check's name and log tail, the ask; delivery *Fix CI · sent 15:44*. Step divider *attempt 3 · Fix CI*, Builder's answer, *CI running again* |
| | | `NMHzc` 11c · Conversation · Fix CI automatically | *Sent Fix CI 1 of 3 automatically* as an event line, Builder's partial fix, the card *CI failed again* with *Fix CI 2 of 3 · sent automatically* and *Turn off for this run*, the automatic message (one failing check, ask *Fix it …*; *Sent automatically · Fix CI 2 of 3*) |
| | | `kLZUJ` 11d · Pull request CI on a run · parts and rules | CI line states (Running, Passed, Failed, Unknown, Fixing). *When CI can't be read*: no pull request, gh not found and not signed in (both with the *gh auth login* chip), couldn't read. Switch and limit states (off, on, counting, limit reached, can't read). Event lines. Rules. Copy keys |

All frames are dark (the board's theme). Exports at scale 1: `.tmp/design/ORC-608/<node>.png` (not committed). Pencil
exports a nested frame without its board's theme, so the frame labels in the exports render in light colours.

Board moves (the person approved them): `C3wC8` grew by the two rows (985 + 2049 px). `hujxu`, `HFXxX`, `zuvUi`, `XGtkg`,
`iUmo3`, `evQ4U`, `i5gEcb`, `ri5ru` and `acFkz` moved down 3034 px, y only. No board overlaps `C3wC8` now.

In the Summary frames the lower Summary sections (Acceptance criteria, Output, Test report) are cut by the window's
scrolling Body, as in 01e `R7iv5`: they sit below the fold on purpose.

## Where CI shows

- **Summary**: a *Pull request* card is the first block of Main, in place of the decision card. It has:
  - a head: `git-pull-request`, *Pull request #48*, the branch, who opened it, and *Open on GitHub ↗*;
  - the CI line: a badge and a count;
  - the checks list;
  - the actions;
  - the *Fix CI automatically* row, under a top border.

  The stroke follows the state: `border` (running, unknown), `danger` (failed), `success` (passed). The Run section's
  last-event line repeats the state in one sentence.
- **Header**: the status line names the CI state (*CI running / failed / passed on #48*, *Fixing CI on #48 · 2 of 3*). One
  primary action by state: *Fix CI* when failed, *Pause* while the agent fixes, *Open pull request* otherwise.
- **Conversation**: event lines for opened, started, passed and not read. A failed CI that nobody has answered is a card
  with *Fix CI*. Once Fix CI is sent (by hand or automatically), the next failure shows as a card again and the answered one
  folds into a line.

## Check rows

- The status icon is lucide 13: `circle-check` `success`, `loader-circle` `text_secondary`, `circle-x` `danger`,
  `circle-dashed` `text_muted` (queued). The name is in `font_mono` 12. The duration is in `text_muted`, then *Details ↗*,
  which opens the check's run on GitHub.
- Failing checks come first, each with its log tail (the last lines, `font_mono` 11.5 on `code_bg`, `radius_md`, the last line
  in `text_primary`), indented under the name. Passed and skipped checks fold into one line: *3 passed · build, …*.
- In the Conversation card a failing row shows only the first log line, inline. The full tail is in the Summary and in the
  message.

## Fix CI

- It sends one message to the run's current agent:
  - the lead (*CI failed on pull request #48. 2 of 5 checks failed:*);
  - for each failing check, its name and the last 40 lines of its log, with ANSI codes stripped;
  - the ask (*Fix them on kon-12-retry and push. Don't change the tests' expectations unless they are wrong.*; with one
    failing check: *Fix it … the test's expectations …*).

  Passed checks are left out.
- The message shows as *Your message*, with the tails as code blocks, so the person sees exactly what the agent got.
  Delivery: *Fix CI · sent 15:44*. An automatic one shows *Sent automatically · Fix CI 2 of 3* with the `repeat` icon.
- If the run has ended, Fix CI continues it at the step that wrote the code (a new attempt of 03 Implement), like *Continue &
  send*. If a turn runs, the message waits for it. Fix CI is disabled while CI runs or while a Fix CI already waits, and its
  tooltip says why.

## Fix CI automatically

- `Toggle/Switch` *Fix CI automatically* · *up to* · `Select` (*1, 2, 3, 5 fixes*, default 3), with a hint on the right
  (*none used*, *1 of 3 automatic fixes used on this run*).
- It is off by default and belongs to the run, not the project. A Fix CI sent by hand doesn't count, and changing the limit
  restarts the count.
- While CI can't be read (10d, 11d *Can't read*) the switch, *up to* and the select are shown off and disabled at 50 %
  opacity, with the hint *Needs gh to read checks*.
- At the limit the run stops sending: `Badge/Warning` *Limit reached*, the event *Fix CI automatically stopped at 3 of 3 · CI
  still fails on #48 · needs you*, and the task goes to *Needs you*. Fix CI by hand still works.
- In the Conversation the card offers *Turn off for this run* (`Button/Ghost`).

## When CI can't be read

- **No pull request**: no card, no CI lines, no Fix CI and no switch. The Summary is as before.
- **gh not found / not signed in**: `Callout/Warning` in the card in place of the checks, with *Check again* (`Button/Secondary`)
  and *Open on GitHub*. The switch is off and disabled (*Needs gh to read checks*). In the Conversation: *CI not read on #48 ·
  GitHub CLI isn't signed in · Check again*.
- **gh failed** (HTTP error, rate limit): `Callout/Danger` *Couldn't read CI* with gh's answer. It retries in a minute, and
  *Check again* retries now.
- Orchester polls CI with gh every 30 s while the pull request is open.

## Copy (namespace for the implementer)

Strings under `task.ci.*`: `title`, `open_github`, `state.{running,passed,failed,unknown,fixing}`,
`summary.{running,passed,failed}`, `passed_fold`, `details`, `fix`, `fix_hint`, `open_checks`,
`auto.{label,up_to,attempts,none_used,used,limit_reached,off_hint,needs_gh,turn_off}` (`auto.attempts` is *{n} fixes*,
`auto.used` is *{used} of {limit} automatic fixes used on this run*),
`gh.{not_found,not_signed_in,read_failed,command}` (`gh.command` is the code chip *gh auth login*), `check_again`,
`header.{running,passed,failed,fixing}`, `event.{opened,started,passed,failed,sent,auto_stopped,not_read}`,
`message.{lead,check,ask,sent,sent_auto}`. `message.ask` has a singular and a plural form (*Fix it* / *Fix them*).

## Words

- *attempt N* is only the run's step attempt (the step dividers and headers, *03 Implement · attempt 4*).
- Fix CI counts its own sends as *Fix CI n of m* and *fixes* (*up to 3 fixes*, *1 of 3 automatic fixes used*), never as
  attempts, so the two counts don't read alike.

## Spacing

Gaps and paddings in the new cards, rows, messages and 11d use `$spacing_xs/sm/md/lg/xl` wherever the value is on the scale.
Values off the scale (2, 6, 10, 14) are kept as plain numbers: they match the board's existing cards (the `[14,16]` card
padding, the 10 px row gaps and the `[8,10]` log tail, as in 01e and 02). Instances keep their components' own spacing.

## Open

- The Pull request card, check row, log tail and CI card are local frames, not components. Shared components would need the
  person's go-ahead.
- No variables were added. The only literal colour is the transparent tab stroke `#00000000`, which came with the copied
  Conversation frames.
- Off-scale spacing (2, 6, 10, 14) follows the board; moving it onto the scale is a board-wide decision.
- *Merge when ready* (merge on GitHub when CI passes) is not designed. 10c leaves merging to GitHub.
- The Summary *Fixing* state is described in 11d, but no Summary frame is drawn for it.
