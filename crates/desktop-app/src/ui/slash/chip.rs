//! The skill chip a composer holds before its message (ORC-325, ORC-634; `Skill chip` `dBGRj` in `06d` `lXQi1`, `07c`
//! `f0b717`, `06c` `DunaJ`): a `graduation-cap` 12 in `text_secondary`, the skill's name in Geist Mono 11
//! `text_secondary` and a 12 `x` in `text_muted` that removes it, 4 apart, on a `surface_raised` pill in a `border`
//! stroke, 2 over and under them and 8 at their sides, 6 before the message's first line. Under the pointer a tooltip
//! names the skill and its published version (`FKUJD`) and says how it reaches the agent ([`SkillReach`]). The skill
//! goes with the message; a sent message shows the same chip without its `x` over its words ([`sent_skill_chip`],
//! `r78GAg` in `OKzZ6`). A plan's changes carry no skill: while the chip holds one there, a quiet line under the box
//! says so ([`plan_held`]).

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Button, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::AgentProvider;
use orchester_workspace::skills::offered::OfferedSkill;

use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    tooltip::Tooltip,
  },
};

/// How a chosen skill reaches the agent the message goes to (ORC-634), as the chip's tooltip says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillReach {
  /// The Orchestrator on Claude Code loads it as the CLI's own skill for a turn of its own.
  ClaudeOwn,
  /// The Orchestrator on Codex gets its text in a marked block: Codex's `$skill` is not used yet.
  CodexText,
  /// A run's agent, of any engine, gets its text in a marked block.
  Text,
}

impl SkillReach {
  /// How the Orchestrator running on `provider` gets a skill.
  pub const fn orchestrator(provider: Option<AgentProvider>) -> Self {
    match provider {
      Some(AgentProvider::ClaudeCodeCli) => Self::ClaudeOwn,
      Some(AgentProvider::CodexCli) => Self::CodexText,
      _ => Self::Text,
    }
  }

  const fn key(self) -> &'static str {
    match self {
      Self::ClaudeOwn => "composer.skill.reach.claude",
      Self::CodexText => "composer.skill.reach.codex",
      Self::Text => "composer.skill.reach.text",
    }
  }
}

/// `field`, the composer's message field, after the chip of the skill `skill` holds while it holds one, in the
/// composer named `id`, whose message reaches its agent as `reach` says. The field keeps its place in the tree either
/// way, and with it its focus.
pub fn with_skill_chip(
  ctx: &mut Ctx,
  (id, reach): (&str, SkillReach),
  skill: &Signal<Option<OfferedSkill>>,
  field: Element,
) -> Element {
  let chip = skill_chip(ctx, (id, reach), skill);
  Row::new()
    .id(format!("{id}-skill-line"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .with_children(chip)
    .child(
      Row::new()
        .id(format!("{id}-message"))
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .child(field),
    )
    .into()
}

/// Under the box of the composer named `id`, while its chip holds a skill and it writes a plan's changes: that they go
/// without one.
pub fn plan_held(ctx: &Ctx, id: &str) -> Element {
  Text::new(&ctx.t("composer.skill.planHeld"))
    .id(format!("{id}-skill-held"))
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
    .into()
}

/// The chip of `skill`, a sent message's, named `{id}-skill-chip`, without its `x`; its tooltip names the version.
pub fn sent_skill_chip(ctx: &mut Ctx, id: &str, skill: &OfferedSkill) -> Element {
  let chip_id = format!("{id}-skill-chip");
  let tip = tooltip(ctx, "composer.skill.tooltip", skill);
  let chip = pill(&chip_id, skill);
  Tooltip::new(chip_id, tip).wrap(ctx, chip)
}

/// The chip of the skill `skill` holds, in the composer named `id`, while it holds one; its `x` empties it.
fn skill_chip(ctx: &mut Ctx, (id, reach): (&str, SkillReach), skill: &Signal<Option<OfferedSkill>>) -> Option<Element> {
  let chosen = skill.get()?;
  let chip_id = format!("{id}-skill-chip");
  let remove_label = ctx.t_args("composer.skill.remove", [("name", &*chosen.slug)]);
  let held = skill.clone();
  let remove = Button::empty()
    .id(format!("{chip_id}-remove"))
    .align_items(Alignment::Center)
    .focusable(false)
    .cursor(CursorIcon::Pointer)
    .describe("a11y_name", remove_label)
    .on_click(move |event: MouseEvent| {
      event.stop_propagation();
      held.set(None);
    })
    .child(icon(Icon::X, IconSize::Sm, PaletteColor::TextMuted));
  let chip = pill(&chip_id, &chosen).describe("reach", reach.key()).child(remove);
  let tip = tooltip(ctx, reach.key(), &chosen);
  Some(Tooltip::new(chip_id, tip).wrap(ctx, chip))
}

/// The pill of `skill` named `chip_id`: its glyph and its name.
fn pill(chip_id: &str, skill: &OfferedSkill) -> Row {
  Row::new()
    .id(chip_id.to_owned())
    .describe("skill", skill.slug.clone())
    .describe("version", skill.version.get().to_string())
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Full.role())
    .child(icon(Icon::GraduationCap, IconSize::Sm, PaletteColor::TextSecondary))
    .child(
      Text::new(&skill.slug)
        .id(format!("{chip_id}-name"))
        .variant(SlashText::Chip.role())
        .nowrap(),
    )
}

/// The words of the tooltip `key` names, for `skill`.
fn tooltip(ctx: &Ctx, key: &str, skill: &OfferedSkill) -> std::sync::Arc<str> {
  ctx.t_args(
    key,
    [
      ("name", skill.slug.to_string()),
      ("version", skill.version.get().to_string()),
    ],
  )
}
