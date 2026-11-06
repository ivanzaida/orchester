//! The skill chip a composer holds before its message (ORC-325, ORC-634, ORC-663; `Skill chip · hover` `GWWGb` and
//! `Skill chip` `jmI0O` in `07` `QnPRC`, the tooltips in `07b` `z1VcW9`): an 11 `graduation-cap` in `text_secondary`,
//! the skill's name in Geist Mono 11 `text_secondary` and an 11 `x` in `text_muted` that removes it, 4 apart, on a
//! `surface_raised` pill in a `border` stroke (`border_strong` under the pointer), 2 over and under them and 8 at their
//! sides, 6 before the message's first line. Under the pointer a tooltip opens over it, 6 clear of the composer's top
//! edge, wraps at 360 ([`SKILL_TIP_WIDTH`]), names the skill and its published version and says how it reaches the
//! agent ([`SkillReach`]). The skill goes with the message; a sent message shows the same chip without its `x`, 8 over
//! its words ([`sent_skill_chip`]). A plan's changes carry no skill: while the chip holds one there, a quiet line under
//! the box says so ([`plan_held`]).

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Button, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::AgentProvider;
use orchester_workspace::skills::offered::OfferedSkill;

use super::{SlashField, argument};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    tooltip::Tooltip,
  },
};

/// How a chosen skill reaches the agent the message goes to (ORC-634, ORC-663), as the chip's tooltip says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillReach {
  /// The Orchestrator on Claude Code loads it as the CLI's own skill for a turn of its own.
  ClaudeOwn,
  /// The Orchestrator on Codex loads it as Codex's own skill (`$<slug>`) for a turn of its own.
  CodexOwn,
  /// A run's agent, of any engine, gets its text in a marked block.
  Text,
}

impl SkillReach {
  /// How the Orchestrator running on `provider` gets a skill.
  pub const fn orchestrator(provider: Option<AgentProvider>) -> Self {
    match provider {
      Some(AgentProvider::ClaudeCodeCli) => Self::ClaudeOwn,
      Some(AgentProvider::CodexCli) => Self::CodexOwn,
      _ => Self::Text,
    }
  }

  const fn key(self) -> &'static str {
    match self {
      Self::ClaudeOwn => "composer.skill.reach.claude",
      Self::CodexOwn => "composer.skill.reach.codex",
      Self::Text => "composer.skill.reach.text",
    }
  }
}

/// The widest the composer chip's tooltip runs before it wraps (`Label` `xuZ4N` in `mhanX`, `07` `QnPRC`).
const SKILL_TIP_WIDTH: f32 = 360.0;

/// `field`, the composer's message field, after the chips `slash` holds: the command with an argument and its choices
/// ([`super::argument`]), and the skill, in the composer named `id`, whose message reaches its agent as `reach` says;
/// the skill chip's tooltip keeps clear of `frame`, the composer's box. The field keeps its place in the tree either
/// way, and with it its focus.
pub fn with_chips(
  ctx: &mut Ctx,
  (id, reach, frame): (&str, SkillReach, &ElementRef),
  slash: &SlashField,
  field: Element,
) -> Element {
  let command = argument::chips(ctx, id, slash);
  let chip = skill_chip(ctx, (id, reach, frame), &slash.skill);
  Row::new()
    .id(format!("{id}-skill-line"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .with_children(command)
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

/// The chip of command `name` in a sent message (`Chip · /watching` `az1pt` in 06 `Ff7AD`), named
/// `{id}-command-chip`: its `glyph` and `/name`, as the composer's command chip shows them, in a `border_strong`
/// stroke.
pub fn sent_command_chip(id: &str, glyph: Icon, name: &str) -> Element {
  let chip_id = format!("{id}-command-chip");
  pill_frame(&chip_id, PaletteColor::BorderStrong)
    .describe("command", name.to_owned())
    .child(icon(glyph, IconSize::Xs, PaletteColor::TextSecondary))
    .child(chip_words(&format!("{chip_id}-name"), &format!("/{name}")))
    .into()
}

/// The chip of the skill `skill` holds, in the composer named `id`, while it holds one; its `x` empties it, and its
/// tooltip opens over it, clear of `frame`, the composer's box.
fn skill_chip(
  ctx: &mut Ctx,
  (id, reach, frame): (&str, SkillReach, &ElementRef),
  skill: &Signal<Option<OfferedSkill>>,
) -> Option<Element> {
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
    .child(icon(Icon::X, IconSize::Xs, PaletteColor::TextMuted));
  let chip = pill(&chip_id, &chosen)
    .describe("reach", reach.key())
    .hovered_style(Style::new().border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong)))
    .child(remove);
  let tip = tooltip(ctx, reach.key(), &chosen);
  Some(
    Tooltip::new(chip_id, tip)
      .over(frame)
      .wrap_at(SKILL_TIP_WIDTH)
      .wrap(ctx, chip),
  )
}

/// The pill of `skill` named `chip_id`: its glyph and its name.
fn pill(chip_id: &str, skill: &OfferedSkill) -> Row {
  pill_frame(chip_id, PaletteColor::Border)
    .describe("skill", skill.slug.clone())
    .describe("version", skill.version.get().to_string())
    .child(icon(Icon::GraduationCap, IconSize::Xs, PaletteColor::TextSecondary))
    .child(chip_words(&format!("{chip_id}-name"), &skill.slug))
}

/// A composer chip's pill named `chip_id`, in a `stroke` line: `surface_raised`, round, 2 over and under its parts
/// and 8 at their sides, 4 between them.
pub(super) fn pill_frame(chip_id: &str, stroke: PaletteColor) -> Row {
  Row::new()
    .id(chip_id.to_owned())
    .flex_shrink(0.0)
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Tight.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(ExtraRadius::Full.role())
}

/// A chip's name `id`, in Geist Mono 11 `text_secondary`.
pub(super) fn chip_words(id: &str, words: &str) -> Text {
  Text::new(words)
    .id(id.to_owned())
    .variant(SlashText::Chip.role())
    .nowrap()
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
