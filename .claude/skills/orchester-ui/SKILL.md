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

## Text through i18n

### Where

- Strings: `crates/desktop/assets/locales/<locale>.json` (nested JSON, flattened by lurq to
  dotted keys). Today only `en.json` ships.
- Loading: `crates/desktop/src/i18n.rs` (`LOCALES`, `install`). A new locale is one new
  file plus one `LOCALES` entry.
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
- `palette.rs`, `typography.rs`, `metrics.rs`, `form.rs` — fill lurq's roles from tokens.
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
| `text` #EEEEEE | `TextPrimary`, `BorderFocus` |
| `muted` #A3A3A3 | `TextSecondary` |
| `subtle` #8F8F8F | `TextMuted`, `BorderStrong` |
| `accent` / `inverse-text` | `Accent` / `TextInverse` — primary action |
| `radius-control` 7 / `radius-panel` 12 / nav item 6 | `RadiusSize::Md` / `Lg` / `Sm` |
| `space-sm/md/lg` 8/16/24 | `SpacingSize::Sm/Md/Lg` |
| stroke 1 / focus stroke 1.5 | `BorderSize::Sm` / `Md` |

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
| `Mono` | 10 | revisions, shortcuts, counts (Geist Mono not bundled yet) |
| `ExtraTypography::SectionHeading.role()` | 15/500 | content section heading ("What changed") |
| `ExtraTypography::CardTitle.role()` | 14/500 | card and panel titles ("Needs you") |
| `ExtraTypography::Overline.role()` | 9/600 subtle | uppercase category label ("REVIEW PULL REQUEST") |
| `ExtraTypography::Metric.role()` | mono 23 | large metric figure |

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
