---
name: orchester-ui
description: Orchester's rules for user-facing text and visual styling in the lurq desktop app — every visible string through i18n keys in crates/desktop/assets/locales, every color, font, text size, spacing, radius and border through lurq theme roles filled from the Pencil design. Use this skill together with `lurq-ui` whenever you add or change anything a user can see in `orchester-desktop` (screen, panel, row, field, button, dialog, empty/error state, window title, tooltip), add or rename a translation key, map a new design value into the theme, or when `tools/check.ps1` reports a "ui literals" violation or `tests/i18n.rs` / `tests/theme.rs` fail.
---

# Orchester UI: i18n and theme tokens

Two rules, both mechanically checked:

1. **Every user-facing string is an i18n key.** Components never contain display text.
2. **Every visual value is a lurq theme role.** Components never contain colors, fonts,
   text sizes, spacing, radii or border widths. Design values live only in
   `crates/desktop/src/theme/`.

Load `lurq-ui` too — it covers component boundaries, interaction and verification. This
skill is the Orchester-specific layer: where things live and how to name them.

## The new app (`orchester-app`)

`crates/desktop-app` (the `crates/desktop-app/design/orchester.pen` design: read it through the `pencil` MCP with
the absolute path of that file in the project's main checkout as `filePath` — the folder holding `.orchester/`, not
your worktree's copy, which Pencil has not open, so the MCP would silently answer from its active tab instead; see
AGENTS.md › Design source; `data/orchester2.pen` is an older design, not used) follows the same two rules with its own
files; the paths and the mapping tables below are the deprecated `orchester-desktop`'s.

- Strings: one file per locale and area, `crates/desktop-app/assets/locales/<locale>/<area>.json`
  (`shell`, `common`, `task`, and a screen's own area such as `tasks` or `task_form`). An area's file
  holds its keys under the area's name and nothing else (`tasks.json` is `{"tasks": {…}}`), so a key's
  first segment names its file and no key is in two files. `build.rs` lists the files and
  `crates/desktop-app/src/i18n.rs` loads them all into lurq's default namespace, so code names a key in
  full (`ctx.t("tasks.newTask")`); a new area is just a new file, in every shipped locale.
- Which area a string goes in:
  - Each area keeps its own words in its own file, even when another area has the same English words.
    A screen's task owns its area's file and edits no other.
  - `common.*` is only for words that are truly shared (Cancel, Save, Copy, Retry) and used by more than
    one area. They are added through the UI-kit task, not by a screen's task.
  - `shell.*` holds the screens' names, the chrome, the sidebar and the status bar.
  - The task page's frame (title, run line, tab names) is `task.*`. Each of its sub-areas has a
    top-level area file of its own: `summary.json`, `conversation.json` and `changes.json`, not
    `task.summary.*`. New task and Edit task are `task_form.json`.
- Design values: only in `crates/desktop-app/src/theme/` (`palette.rs`, `typography.rs`, `metrics.rs`,
  `form.rs`, `shadow.rs`); call sites use lurq roles and the `ExtraColor`, `ExtraTypography`,
  `IconSize`, `ExtraSpacing` and `ExtraRadius` roles there. Shared roles belong to the UI kit; a value
  only one area uses goes in that area's own `theme/<area>.rs` (`startup.rs`, `shell.rs`), registered
  with one line in `theme/mod.rs` and `typography.rs`. A structural size (a sidebar width, a tab
  height) is a named constant next to the component that owns it, with its Pencil node id.
- A `Message` of `orchester-workspace` is shown with `ui::message::message_text`, which reads its key in
  the `workspace` namespace; never `ctx.t` with its key.
- Tests: `cargo test -p orchester-app --test i18n -- locales area every_key` (load, parity per area, own
  keys only, every key the code names has its area's file and exists) and
  `cargo test -p orchester-app --test theme -- roles extra_color spacing`.
- `tools/check.ps1`'s "ui literals" check covers `crates/desktop-app/src` too (its `theme/` exempt).
- The UI kit (`crates/desktop-app/src/ui/`, Kontur's shared components `ASNYj`/`DMMLd`) comes first on the reuse
  ladder: buttons, fields, selects, menus, popovers, tabs, toggles, status/priority labels, chips, notices, cards,
  tables, empty/loading/error states, Markdown, code box, tooltips and cut text, avatars, breadcrumbs, the
  unavailable reason. Its own words are in `en/ui.json` (`ui.*`), the shared Copy and Try again in `common.*`; extra roles add `ExtraShadow`, `ControlHeight` and the
  `DISABLED_OPACITY` fader. Tests: `cargo test -p orchester-app --test ui_kit <filter>`;
  every control in its states: `cargo run -p orchester-app --example ui_gallery` (MCP like the app).
- Grouped rows (`ui::row_group`: `RowGroup`, `GroupRow`, `tag`, `note`; roles `GroupText`) draw a titled group of rows
  in one outline, as the redesigned Pipelines screens do (`Pipelines · redesign` `evQ4U` in the new `orchester.pen`):
  a row's label and hint, and at its end its control, its value or a chevron to a page; a radio row leads with
  `toggle::radio_mark`. Their notices are `ui::notice::Notice::flat` (no stroke, a 14 icon, actions under the words).
  Pipelines' own values are `PipelinesText` / `PipelinesSpacing` (`theme/pipelines.rs`). A
  pipeline's page is one route with its tab and what its Steps column shows (`routes::PipelineView`): the canvas,
  the step inspector, the settings page and the history panel are gone.

## Confirmations and approvals in a modal

A question that confirms or approves something the person just asked for — delete, discard, mark released, merge,
any step that cannot be undone — opens a **modal dialog** over the page (`ui::backdrop` with the dialog roles:
`DialogTitle`, `Explanation`, the action row). Never insert a warning, panel or confirm row into the page for it:
a state that appears in the layout pushes everything under it around, and that is worse than a modal (the person,
2026-10-28). This holds for Pencil designs and for the app.

- The dialog says what will happen and what is lost, with the safe action (*Cancel*) and the action itself
  (`danger` when destructive); Esc and the backdrop cancel. Extra input the action needs (an optional commit, a
  message) sits in the dialog.
- Still pages, not dialogs: creating and importing things (*New …*, *Import …*), forms and editors.
- Not confirmations: work that waits for the person where it is (Needs you cards, a run's approval card in its
  Conversation) stays where it is.

## Text through i18n

### Where

- Strings: `crates/desktop/assets/locales/<locale>.json` (nested JSON, flattened by lurq to
  dotted keys). Today only `en.json` ships.
- Keys that `orchester-workspace` names (its `Message`s, statuses, months, units) live in
  `crates/workspace/assets/locales/<locale>.json` instead, never in both files. An app loads them
  under the namespace `workspace` (`orchester_workspace::i18n::NAMESPACE`); the old app also loads
  them into its default namespace, so its screens read them with `ctx.t`.
- Loading: `crates/desktop/src/i18n.rs` (`LOCALES`, `install`). A new locale is one new
  file plus one `LOCALES` entry, in both crates.
- In components: `ctx.t("tasks.newTask")`, `ctx.t_args("runs.finishedAgo", [("time", t)])`.
  Outside a component (window title, menus): `app.i18n().t(...)`.

### Key naming

- Namespace first, from the design handoff (`docs/design/orchester-workspace.md`): `app`,
  `common`, `nav`, `workspace`, `tasks`, `runs.status`, `pipelines`, `instructions`,
  `variables`, `tools`, `projectImport`, `secrets`, `access`, `recovery`. Add a namespace
  only for a new product area.
- Leaves are camelCase and name the *meaning*, not the words: `instructions.publishVersion`,
  `secrets.usedBy`, `access.allowDeployment` — not `instructions.publishV3Button`.
- Shared words (Cancel, Save, Close, Retry) go in `common.*`; reuse them instead of
  duplicating per screen.
- Keep JSON grouped by screen/area in the same order as the UI reads.

### Composing text

- One key per complete message. Never concatenate translated fragments or build sentences
  with `format!` — word order differs between languages. Put variables inside the message:
  `"finishedAgo": "Finished {{time}} ago"` → `ctx.t_args("runs.finishedAgo", [("time", t)])`.
  Placeholders are `{{name}}`, not `{name}`.
- lurq i18n has no plural rules. When a count changes the wording, use separate keys
  (`tasks.activeCount.one` / `tasks.activeCount.other`) and pick one in code.
- Errors shown to the user are keys too: map core errors to an i18n key at the UI boundary;
  never display an error's `Display` text or a debug string.

### Not translated

User-authored and runtime data: project/task/pipeline names, prompts, file paths, branch
names, commit hashes, secret *names*, model ids, numbers and costs as values. Developer logs
and element `id`/`class`/`describe` attributes. Icon names.

### Enforced by

- `tests/i18n.rs`: every shipped locale has exactly the keys and placeholders of `en.json`,
  and every `ctx.t("…")` / `ctx.t_args("…")` key used in `src/` exists (lurq would silently
  show the raw key otherwise). Run: `cargo test -p orchester-desktop --test i18n locale`
  and `... --test i18n every_key`.
- `tools/check.ps1` → "ui literals": `Text::new("…")`, `Button::new("…")`,
  `.placeholder/.with_title/.tooltip/.label("…")` fail the check.

## Visuals through theme roles

### Where

- `crates/desktop/src/theme/tokens.rs` — raw values from Pencil (`data/orchester.pen`,
  read via the `pencil` MCP `get_variables`, or node properties via `batch_get`). Private to
  the theme module.
- `palette.rs`, `typography.rs`, `metrics.rs`, `shadow.rs`, `form.rs` — fill lurq's roles from tokens.
- `theme::ExtraColor`, `theme::ExtraTypography` — design values without a built-in lurq role,
  registered as lurq `Extra` roles. Call sites use these enums, never raw extra names.
- `tests/theme.rs` — asserts roles resolve to the Pencil values; update it when a mapping
  changes.

### Current mapping

| Pencil | lurq role |
| --- | --- |
| `canvas` #101010 | `PaletteColor::SurfaceBase` |
| `sidebar` #141414 | `SurfacePanel`, `SurfaceInput` (fields are sidebar-filled) |
| `surface` #1A1A1A | `ExtraColor::Surface.role()` — cards, panels on canvas |
| `raised` #222222 | `SurfaceRaised` — buttons, pipeline steps |
| `hover` #2C2C2C | `AccentMuted` — hover, selected nav item, status label |
| `border` #363636 | `Border` |
| `hairline` #2B2B2B | `ExtraColor::Hairline.role()` — row and section dividers |
| backdrop #000000A6 (`a8U756`) over the screen only, not the title or status bar | `ExtraColor::Scrim.role()` — behind dialogs, through `ui::backdrop` |
| `text` #EEEEEE | `TextPrimary`, `BorderFocus` |
| `muted` #A3A3A3 | `TextSecondary` |
| `subtle` #8F8F8F | `TextMuted`, `BorderStrong` |
| `accent` / `inverse-text` | `Accent` / `TextInverse` — primary action |
| `danger` #F87171 | `PaletteColor::Danger` — only the *Unreadable* status label's icon and word (`mGbYs`) |
| `radius-control` 7 / `radius-panel` 12 / nav item 6 | `RadiusSize::Md` / `Lg` / `Sm` |
| `space-sm/md/lg` 8/16/24 | `SpacingSize::Sm/Md/Lg` |
| stroke 1 / focus stroke 1.5 / selected tab underline 2 | `BorderSize::Sm` / `Md` / `Lg` |
| form group gap 22 (`xT2ex`, `dCLMl`) | `ExtraSpacing::Form.role()` |
| lifecycle dialog width 880 at most, 32 from the window's side edges (`um9B1`, `KS2RU`, `BovYZ`) | `theme::LIFECYCLE_DIALOG_WIDTH` / `ExtraSpacing::DialogMargin.role()` (backdrop padding) |
| Summary column padding beside its scroll gutter, 32 less the 12px gutter (`JV8S8` in `ZVdRa`) | `ExtraSpacing::BesideGutter.role()` |
| what a check row says under its title, indented past its icon and gap (`gzztk` in `LMrbK`) | `ExtraSpacing::CheckIndent.role()` |
| note field padding (`NbNN0`, the changes a plan needs) | `ExtraSpacing::TextArea.role()` |
| agent-written Markdown (a plan under review, 02.11): headings step up from the body, code in the `code-*` colors, quotes and tables in the chrome's borders | lurq `ThemeMarkdown` from `theme::markdown` (headings 14/13/12 medium); call sites pass the text style (`AgentText`) |
| select menu radius 11 / padding 4 (`u0ByM`) | `ExtraRadius::SelectMenu.role()` / `ExtraSpacing::SelectMenu.role()` |
| switch track radius 8, knob round (`K8ItM`) | `ExtraRadius::Pill.role()` |
| unparsed transcript lines at 60 % (`Ane9j`) | `theme::UNPARSED_OPACITY` |
| permission option that cannot be chosen at 50 % (`BNliD` in `T1rjv`) | `theme::UNAVAILABLE_OPTION_OPACITY` |
| switch whose value is fixed: track and label faded together at 45 % (`lW7Sp` in 04.B `W0Owo`) | `theme::LOCKED_SWITCH_OPACITY`, applied by `ui::switch::Switch::locked` |
| loader (`lucide/loader`) turning while shown, one turn a second (the design draws it still) | `theme::LOADER_TURN`, applied by `ui::icon::icon` through `ui::loader`; each window tree calls `loader::install` |
| switch on: `accent` track and stroke, `inverse-text` knob at the right (`E1Y9KK`) | `Accent` / `TextInverse`, in `ui::switch` |
| switch state label 12 `text`, 10 from the track (`q43RMP` in `lW7Sp`, `q4Nelz`); with a detail after it, track, label and detail 12 apart (`k1QWD`) | `ui::switch::Switch::emphasized` (`ExtraSpacing::Inline`), `Switch::detail` (`ExtraSpacing::Stack`) |
| why an option cannot be chosen: indented 22 past a compact radio and its gap, an 11px lock (`J5RohL`, `L1B0m`, `OoLYh` in 04.B `W0Owo`) | `ExtraSpacing::ReasonIndent.role()` / `IconSize::Reason` (`ExtraTypography::IconReason`), in `ui::option_card::unavailable_reason` |
| a digit of a tab count in Geist Mono 10 (0.6 em), the room the Open tab keeps for its count (`tidLB` in `DF8II`) | `theme::COUNT_DIGIT_WIDTH`, through `ui::tab::Tab::count_room` |
| button that cannot be pressed, primary or secondary: its own colors at 40 % (`kLJJb`, `HN7cO`) | `theme::DISABLED_OPACITY`, applied by `ui::button::ActionButton` |
| select menu overlay scrollbar: 4px `border` thumb, radius 2, inset 1 (`g3poI`) | `theme::select_menu_scrollbar()` |
| scroll gutter: a reserved 12px gutter, 4px `border` thumb, radius 2, 4 from each side and from the viewport's top and bottom (`nf9DG` in `JuVaA`) | `theme::gutter_scrollbar()` |
| transcript command box: lines 4 from the viewport's top and bottom (`Z3M6Nz` in `uA3Sh`); horizontal thumb 4px `border`, radius 2, overlaid 1 from the box's bottom and 12 from the viewport's ends (`hpzPu` in `G9skx`) | `ExtraSpacing::CommandLines.role()` / `theme::command_scrollbar()` |
| a long value's box in the approval dialog: 12 lines with its padding, then its lines scroll (`E5NBo` in `bjrgu`); the 04.H9 shell string (`bwWuT`) shares it; its thumbs in 6px lanes of their own inside its 6px padding, from the viewport's start (`e0vyng`); 12 under its label | `theme::APPROVAL_VALUE_MAX_HEIGHT` (232) / `theme::value_box_scrollbar()` / `ExtraSpacing::Stack.role()`, through `ui::value_box` |
| quiet text action beside a label, e.g. a value's Copy (`feLwP`): 22 high, 6 at each side and between icon and label, no fill or border, `muted` icon and label; under the pointer the `hover` fill and `text` (`oriyY`); focus ring in a form | `ui::button::ActionButton::quiet` (`TextSecondary` / `TextPrimary`, `AccentMuted`, `ExtraSpacing::Tight`) |
| operations popover drop shadow 0 8 24 #00000066 (`E1msJS`) | `ExtraShadow::Popover.role()` |
| `focus-ring` #EEEEEE / `focus-gap` #101010 (`xURvJ`) | `ExtraColor::FocusRing.role()` / `ExtraColor::FocusGap.role()` |
| focus ring, outer / inset: spreads `focus-width` 2 and `focus-extent` 4 (`xURvJ`, `Oz9EN`) | `ExtraShadow::FocusRing.role()` / `FocusRingInset.role()`, applied only through `ui::pressable::Ring` |
| a focused control that is only words (a link, a disclosure line): 2 of `border_focus` outside them, radius 3 (`Mjhop`) | `ExtraShadow::LinkFocusRing.role()` / `ExtraRadius::Link.role()`, applied only through `ui::pressable::Focus::Link` |
| room inside a control with the inset ring, so the ring and its gap never touch its content: `focus-extent` 4 (`Oz9EN`) | `ExtraSpacing::FocusInset.role()`, in `ui::switch::Switch::inset_ring` |
| `code-bg`, `code-text`, `code-keyword`, `code-string`, `code-function`, `code-variable`, `code-constant`, `code-comment` (`vEZ42`) | `ExtraColor::CodeBackground` / `CodeText` / `CodeKeyword` / `CodeString` / `CodeFunction` / `CodeVariable` / `CodeConstant` / `CodeComment` — code and diff only |
| `diff-add-bg` / `diff-add-text`, `diff-remove-bg` / `diff-remove-text` (`vEZ42`) | `ExtraColor::DiffAddedBackground` / `DiffAddedMark`, `DiffRemovedBackground` / `DiffRemovedMark` |
| `canvas` fully transparent, where the tab strip's edge fade ends (`xpSC0`) | `ExtraColor::CanvasClear.role()` |
| macOS traffic lights #FF5F57 / #FEBC2E / #28C840 (`R3RtGs` in `V70DN`) | `ExtraColor::TrafficClose` / `TrafficMinimize` / `TrafficZoom` |
| Usage dashboard `chart-series-1/2/3` #3987E5 / #D95926 / #199E70 (Codex, Claude Code, a third CLI) and the `#FFFFFF0F` hatching (`C2pTYF`) | `ExtraColor::ChartSeries1` / `ChartSeries2` / `ChartSeries3` / `ChartHatch`; canvas charts draw with `theme::chart::ChartStyle` and the geometry in `theme::chart` |
| title bar tab avatar and close button radius 4 (`b9QQb`, `OvWLh` in `nAKyb`) | `ExtraRadius::TabControl.role()` |
| switcher row *Open in new tab* radius 5 (`NcJQ0`) | `ExtraRadius::RowAction.role()` |
| conversation *Show* segment radius 5, usage period style (toolbar of `RrScB`) | `ExtraRadius::Segment.role()` |
| conversation working line, indented 40 to a chat message's text past its 28 avatar (`VCcdr` in `kpnmP`) | `ExtraSpacing::TimelineIndent.role()` |
| round ends up to 32 high: chat avatar 28, composer pills 28 and send button 32 (`qU0XW`, `zQwSy`, `U0cjVo`); the composer box radius 18 (`n0wdPC`) | `ExtraRadius::Round.role()` / `ExtraRadius::Composer.role()` |
| a tool chip's padding over and under its line 5 (`a7zbJ`); 4 between the rail strip's badges and the composer's send chevron and button (`fFDrw`, `eI8en`) | `ExtraSpacing::Chip.role()` / `ExtraSpacing::Snug.role()` |
| the composer dock's room above its box 4, under it and the closed box's side padding 18 (`CoGTj`, `W9LMuM`) | `ExtraSpacing::ComposerTop.role()` / `ExtraSpacing::ComposerEdge.role()` |
| a rail step that has not started, at 70 % (`r27KI` in `K4AOD`) | `theme::NOT_STARTED_OPACITY` |
| Browser window (`lcNez`): switcher initial 10.5/600, address mono 12, a page dialog's message 13/1.5, the agent's name on its mark 11/500 `text_inverse`, the file dialog's hint 13/500 and 12 `text_secondary` (`FCera` in `03b` `TWpyp`); switcher gap 7, activity line sides 14, dialog padding 20, the hint's sides 18; the dots' and the agent ring's 1.5 stroke; the tab strip's fade to clear `surface_base` | `BrowserText::*` / `BrowserSpacing::*` / `BrowserBorder::Ring` / `BrowserColor::FadeClear` (`theme/browser.rs`) |

| `TypographyStyle` | Size/weight | Used for |
| --- | --- | --- |
| `Heading` | 30/500 | page title |
| `Title` | 16/500 | section title ("Tasks") |
| `Label` | 13/500 | panel and card headings |
| `Body` | 13 | body text, navigation |
| `Description` | 12 muted | supporting copy |
| `FieldLabel` | 12 muted | field labels, field values use it with `TextPrimary` |
| `Caption` | 11 muted | metadata, counts, property rows |
| `Button` | 12/500 | button labels |
| `Link` | 11/500 | inline actions ("Answer question →") |
| `Mono` | 10 | revisions, shortcuts, counts (Geist Mono) |
| `ExtraTypography::SectionHeading.role()` | 15/500 | content section heading ("What changed") |
| `ExtraTypography::CardTitle.role()` | 14/500 | card and panel titles ("Needs you") |
| `ExtraTypography::ReadBack.role()` | 10 subtle | a value read back on one line under a field ("≈ 14M tokens", `n5Wrl` in `W1Dq6`) |
| `ExtraTypography::AxisLabel.role()` | 10 subtle | a day on a chart's x axis ("Wed 24", `X label` in `C2pTYF`); figures and clock times keep `Mono` (`e5PbG`) |
| `ExtraTypography::ReviewHeading.role()` | 14 | column heading of the scheduling review ("Task priority", `Y68u8` in `W1Dq6`) |
| `ExtraTypography::Overline.role()` | 9/600 subtle | uppercase category label ("REVIEW PULL REQUEST") |
| `ExtraTypography::Metric.role()` | mono 23 | large metric figure |
| `ExtraTypography::DialogTitle.role()` | 20/500 | dialog title (`bhcaX`) |
| `ExtraTypography::Explanation.role()` | 12 muted, 1.6 | dialog explanation (`ClBsl`) |
| `ExtraTypography::NoticeHeading.role()` | 12/500 | emphasized notice heading, dialog option title (`T9TyR`) |
| `ExtraTypography::ScreenTitle.role()` | 28/500 | project screen title below its topbar ("Tasks", `eQKWX`) |
| `ExtraTypography::FormLabel.role()` | 12/500 | new task form field label (`Pg9E7`) |
| `ExtraTypography::StatusLabel.role()` | 11/500 | status label text (`DeizF`) |
| `ExtraTypography::NavCount.role()` | mono 11 muted | count beside a navigation item (`LTXft`) |
| `ExtraTypography::Brief.role()` | 13, 1.7 | task brief text (`E9moJ`) |
| `ExtraTypography::CanvasLabel.role()` | mono 9, spaced | pipeline canvas label ("ON TASK START", `wRjZD`) |
| `ExtraTypography::MonoTag.role()` | mono 9 muted | tag beside an inspector title ("STEP 05", `kAJZA`) |
| `ExtraTypography::MonoValue.role()` | mono 11 | key/value property value, slug (`yeeIR`, `cJsA7`) |
| `ExtraTypography::MonoField.role()` | mono 12 | path in a field, e.g. the project folder (`ObbY7` in `xZAZc`, `F5KwDR`) |
| `ExtraTypography::MonoArea.role()` | mono 12, 1.6 | `NAME=value` lines in a mono text area, e.g. the project's environment (`f0e2Hv` in `BGMgO`) |
| `ExtraTypography::ChangeLine.role()` | 12, 1.4 | a change of a pipeline version in the version history (`o6to5` in `DOroV`) |
| `ExtraTypography::Instructions.role()` | 12, 1.65 | step instructions (`v1Ky8`) |
| `ExtraTypography::Hint.role()` | 10 muted, 1.5 | explanation under a field (`k66CB`) |
| `ExtraTypography::VersionFigure.role()` | 18/500 | version in the publish confirmation (`p8L552`) |
| `ExtraTypography::Summary.role()` | 13 muted, 1.6 | explanation under an empty state title (`xIG73`) |
| `ExtraTypography::InspectorTitle.role()` | 18/500 | inspector panel title (connection name, `kU9QO`) |
| `ExtraTypography::CompactStatusLabel.role()` | 10/500 | status label in a list row (`v7McTX`) |
| `ExtraTypography::IconSelect.role()` | lucide 14 | select chevron and option check (`eASPF`, `ClxwV`) |
| `ExtraTypography::RowTitle.role()` | 15/500 | pull request list row title (`G2kdX` in `QHBNZ`) |
| `ExtraTypography::IconRow.role()` | lucide 20 | leading icon of a pull request list row (`r0ZlXX`) |
| `ExtraTypography::PullRequestTitle.role()` | 24/500 | pull request title (`CNO65` in `Rrmhf`) |
| `ExtraTypography::PullRequestContext.role()` | mono 11 muted | pull request context line (`ADELW`) |
| `ExtraTypography::FileListHeading.role()` | 10/500 muted | changed file list heading (`UwSNk`) |
| `ExtraTypography::FileName.role()` | 12 | changed file name (`qkwbj` in `j9NDM`) |
| `ExtraTypography::DiffPath.role()` | mono 14/500 | path of the diffed file (`rxiPH`) |
| `ExtraTypography::DiffLineNumber.role()` | mono 11 `code-comment` | diff line numbers (`b8bSop`, `U4wNN`) |
| `ExtraTypography::Code.role()` | mono 12, 1.6, `code-text` | diff code and change mark; tokens color with `ExtraColor::Code*` (`DQMrs`) |
| `ExtraTypography::Output.role()` | mono 11 muted, 1.6 | command output in the agent transcript (`mMUyR` in `N4SlQq`) |
| `ExtraTypography::IconEntry.role()` / `IconEvent` / `IconState` | lucide 13 / 14 / 20 | transcript entry glyph (`DQmo9`), activity event glyph (`xQYdP` in `mBkeJ`), tab state icon (`qgZru` in `hig8O`) |
| `ExtraTypography::RuleTitle.role()` | 20/500 | title of the instruction being edited (`sjJOc` in `yRKeF`) |
| `ExtraTypography::PromptText.role()` | mono 12, 1.8 | instruction prompt, edited and resolved (`g2Tkg`, `iJdsS`) |
| `ExtraTypography::VariableName.role()` | mono 20/500 | variable shown in the catalog (`Z69wX` in `EfTiT`); skill being edited (`ERFw4` in `ecJtg`) |
| `ExtraTypography::ContextNote.role()` | 9 muted | preview execution context notes (`HvDGw`, `utiRC` in `TnjFo`) |
| `ExtraTypography::CellLabel.role()` | 11 muted, 1.5 | table cell label that wraps beside its icon (worktree state `VHSnL` in `Py65X`); a note between the scheduling review's settings (`q6NwTY` in `W1Dq6`) |
| `ExtraTypography::WrappingError.role()` | 11 `text`, 1.5 | field error that wraps beside its icon, e.g. why a dialog's action is unavailable (`foNvP` in `glurU`, `eeksu` in `XIEn8`); through `ui::field::wrapping_field_error` |
| `ExtraTypography::WrappingProperty.role()` | 11 `text`, 1.6 | long remote MCP Authentication value that wraps in the inspector (`K8qeZv` in `a4lm6D`) |
| `ExtraTypography::InspectorNote.role()` | 10 muted, 1.6 | note under an inspector's details or actions (model source `xK9v9`, status detail `Thdx2`) |
| `ExtraTypography::EnforcementSetting.role()` | 12 text, 1.25 | setting name in the agent enforcement panel (`vgkrV` in `CIeqc`) |
| `ExtraTypography::EnforcementMechanism.role()` | mono 10 muted, 1.3 | mechanism beneath an enforced setting (`x4ZFi` in `CIeqc`) |
| `ExtraTypography::MonoTitle.role()` | mono 12 | identifier as a list row's title, e.g. a secret name (`B1zGN` in `vxAMD`) |
| `ExtraTypography::MaskedValue.role()` | mono 13 muted | a stored secret shown only as its mask (`FNlaR` in `CQxCb`) |
| `ExtraTypography::ColumnHeading.role()` | 9 muted | uppercase heading of a table column, e.g. "SOURCE / VERSION" (`gHiGc` in `WcIvG`) |
| `ExtraTypography::PlanTitle.role()` | 19/500 | title of the plan under review (`J7irj` in `x3lVC`) |
| `ExtraTypography::ReviewNote.role()` | 12, 1.6 | a note written to an agent, e.g. the changes a plan needs (`DDXc5` in `NbNN0`) |
| `ExtraTypography::AgentText.role()` | 12, 1.6 | text an agent wrote: a message body, a plan under review (`FP6BD` in `J5MIT`); also the Markdown body |
| `ExtraTypography::TabInitial.role()` | 9/600 | a project's initial in a title bar tab's avatar (`T0R2Hv` in `nAKyb`) |
| `ExtraTypography::Criterion.role()` | 12, 1.5 | an acceptance criterion and its number (`TextMuted`) on a task's Summary (`j0Ye4t`, `a88TT` in `tqn5P`) |
| `ExtraTypography::TimelineMessage.role()` | 13, 1.6 | what a message in the conversation timeline says, a person's or an agent's (`V17jfc`) |
| `ExtraTypography::ResultHeading.role()` | 12, 1.5 | what answered at a server address in Add server (`q2uPd` in 10.02) |
| `ExtraTypography::FirstPassword.role()` | mono 14, spaced 0.5 | a first password shown once (`pUTsa` in 10.08 B) |
| `ExtraTypography::Caution.role()` | 11 `text`, 1.5 | a caution that wraps, e.g. that a first password is shown only once (`nSpO0` in 10.08 B) |
| `ExtraTypography::HintHeading.role()` | 11/500 | a server's name in the status bar server item's hint (`e8ZUC` in 10.43 A) |

Pick the role by *meaning* in the design, then check the size matches. If two roles fit,
use the one the same element uses on other screens.

### Usage

```rust
Column::new()
  .background(ExtraColor::Surface.role())
  .border_inside(BorderSize::Sm, PaletteColor::Border)
  .rounded(RadiusSize::Lg)
  .padding(SpacingSize::Lg)
  .spacing(SpacingSize::Sm)
  .child(Text::new(&ctx.t("recovery.runnerOffline.title")).variant(TypographyStyle::Label))
  .child(Text::new(&ctx.t("recovery.runnerOffline.body")).variant(TypographyStyle::Description))
```

Color a text with `.color(PaletteColor::…)` only when the design deviates from the
role's default color.

### When the design has a value with no role

Do not inline it, and do not reuse a role whose meaning is different just because the
number matches.

1. Check whether the value is a Pencil variable (`get_variables`) or a one-off on a node.
2. Add it to `tokens.rs` with the Pencil node id in a comment, and map it: a color becomes
   an `ExtraColor` variant, a text style an `ExtraTypography` variant. lurq (0.21+) also has
   `Extra` roles for `RadiusSize`, `SpacingSize` and `BorderSize`; add an app enum the same
   way when the first such value is needed. Weights take any CSS value (`FontWeight::SemiBold`,
   `FontWeight::Numeric(n)`) and `TextStyle::letter_spacing` is available.
3. Update the mapping tables above and `tests/theme.rs` in the same change.
4. If lurq still cannot express it, stop and report it instead of working around the gap;
   lurq is maintained alongside Orchester and gets the missing capability.
5. Values the audit (`docs/evidence/design-consistency-audit-2026-09-23.md`) or the
   design shows inconsistently (e.g. status label radius 5 vs nav item 6): use the role,
   and mention the inconsistency in the report rather than adding a near-duplicate token.

Structural dimensions (a sidebar width, an inspector width, a control height) are not
styling tokens, but they still must not be scattered: define a named constant once, next to
the component that owns the layout, with the Pencil node id.

### Enforced by

`tools/check.ps1` → "ui literals" fails on, anywhere in `crates/desktop/src` outside
`theme/`: hex color strings, `Color::from_hex/rgb/rgba/new`, `TextStyle { … }`,
`Text::styled`, `font_family`, `font_size`, and numeric literals passed to
`padding*`, `spacing`, `gap`, `rounded`, `corner_radius*`, `border*`. Fix the code, never the
pattern list — changing the rules needs the user's approval.

## Verify

Run `powershell -File tools/check.ps1` and the affected `tests/i18n.rs` / `tests/theme.rs`
tests by name. Then hand the screens you changed (with their Pencil node ids) to a
**separate agent** that follows `.claude/skills/design-verification/SKILL.md`: it compares a
Pencil screenshot of each design node with an app screenshot of the same state at 1440 × 960
and reports every mismatch. Your own screenshots do not count. The work is done only when that
verifier reports no blocking or major findings; fix and re-verify until it does.

Before building a screen, take the Pencil screenshot of its node yourself and build against
it, including what the frame does *not* contain (e.g. no native title bar, no outer border).
