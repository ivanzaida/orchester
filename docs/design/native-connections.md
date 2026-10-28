# Native connections in the new app (ORC-166)

The frames of the native Anthropic API connection that ORC-148 built in `crates/desktop-app` (`screens/connections/`
`native_form.rs`, `add_agent/native.rs`, `inspector/native.rs`, `inspector/enforced.rs`, `edit_agent/native.rs`,
`screens/task/cards/approval/request.rs`). They adapt the old app's native group 04.H1–H11 to the patterns of `10b`
(`hc1vS`, Add provider form) and `05b` (`PGwka`, connection page). Every word is the app's (`connections.native.*`,
`connections.apiProvider.*`, `connections.result.*`, `cards.approval.*`); the sample data (Kontur, `ANTHROPIC_API_KEY`,
Builder, Reviewer, KON-12) is illustrative.

## Where

Desktop redesign file `orchester.pen`, board **Project settings · redesign** (`XGtkg`), frame
**Row 8 · ORC-166 · native connections** (`OyEXT`), placed inside *Row 7 · connection pages* (`dl1jd`) to the right of
04c Share. A new row under Row 7 would have overlapped *App settings · redesign*, which starts 121 px below the board;
the person chose this placement on 2026-10-28. The board's width is now `fit_content` (25920), so it grows sideways
only; its height is unchanged (7586), and every window is 1440 × 1000 so Row 7 stays 1037 high.

## Frames

| Frame | Id | Shows |
|---|---|---|
| 10c · Add provider · Anthropic API | `Vze07` | Empty, as it opens: Anthropic API chosen, OpenAI API locked; presets Official chosen, DeepSeek and Custom locked; suggested name; no key chosen; no models; default model None; fallback off; *Save unavailable: Choose the secret holding the API key, or New secret….*, Save disabled |
| 10d · Add provider · Anthropic API | `fiTP0` | Filled: key secret picked, two models added, default model chosen; *After saving, test the connection on its page; then choose it in an agent profile.*, Save offered |
| 10e · Add provider · Anthropic API | `XiKwd` | Save pressed by MCP input: the person-only notice (`connections.apiProvider.personOnly`) in the action bar; Save stays for the person |
| 10f · Add provider · Anthropic API | `S63PcM` | 10d in the light theme |
| 05e · Native connection | `mAAXA` | Connected: head with Test connection and More; Connection group (provider, preset, endpoint, key secret by name, execution, provider fallback) with the sent-to line; Models with Edit models and Fetch models, source, limits, efforts and *Default · high* |
| 05f · Native connection | `SOXLQ` | The same page scrolled down: Models, Enforced by Orchester (seven rows, two *Not offered* with why) and its note, Used by |
| 05g · Native connection | `CGs18` | Key rejected: the warning sentence and the Test connection result in place, *Test failed · api.anthropic.com refused the key.* |
| 05h · Native connection | `frU6n` | Test connection passed: *Test passed*, the models the endpoint listed *just now* |
| 05i · Native connection | `Z7hDL` | 05e in the light theme |
| 05j · Native edit connection | `k8IU1` | Edit connection: name as it is, preset, base URL, key, models (one removed), default model, default effort, fallback; *Not saved: Reviewer uses claude-sonnet-5-5, which is no longer listed. Switch to another model first.* |
| 15 · Native command approval | `DvSDJ` | The task conversation's approval card for a Claude Code command next to a native agent's: Shell (`pwsh · -NoProfile -NonInteractive -Command`) and Time limit (`120 s · then Orchester stops it`) after the working directory; *Deny with a message* unavailable with the app's reason |
| 05k · Native states and notes | `EbPSP` | Every status with its sentence (not checked, key missing, key rejected, billing problem, endpoint unreachable, connected); key missing's head (Edit connection first, Test connection unavailable) and *API key · secret missing*; the Checking…, found-a-problem and failed results; *No models yet*; the key created in place (New secret… with name and key); the locked cards' whole reasons |

Exports: `.tmp/design/ORC-166/<id>.png` at scale 1 (not committed).

## Decisions

- **Locked cards keep their strength**, as the app's compact `OptionCard` does: a lock at the title, the short reason as
  the card's line (*native · not in this version*, *not offered on this API*, *in a later version*). The whole reason is
  the card's accessible reason only; 05k lists it. (10b draws its locked card at 50% opacity; the app does not.)
- **Base URL is an ordinary row**, not folded under *Advanced*: the app has no fold. Its (i) says to change it only for
  a proxy.
- **Default effort appears only in Edit connection**, while the default model is one the endpoint listed with efforts;
  Add provider never shows it (no stored models there).
- **The key created in place** (*New secret…* with its name and key) is drawn in 05k, not in a window: with it the form
  no longer fits a 1000-high window, and the windows may not grow (see *Where*).
- **Approval card**: the redesign file has no approval card frame yet, so 15 draws the CLI card from
  `request.rs` for comparison next to the native one; the native rows are highlighted.
- Status dot colours follow the tones: connected success, key missing and billing warning, key rejected and endpoint
  unreachable danger, not checked muted; the status sentence is in `warning` for every problem, as `sentence_tone` does.
- **Control heights are plain numbers** (buttons and inputs 28, Fetch models and model pills 24), as in `10b`: Pencil
  does not apply a `$control_height_*` variable to a height, and the controls collapse to their text.

## Not drawn

- Add provider's base URL error and the model field's error: the same as `10b`'s.
- The Checking… head (Test connection loading): Button/Secondary/Loading.
