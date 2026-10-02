//! What Orchester could not enforce for a native agent on this run (04.H11 `ZvWcK`, `nHi8z`), under
//! the readiness card: network stayed on although its permissions ask for it off, and its listed
//! commands ran with the person's own access. The native counterpart of the CLI's *Network stays on*
//! line. Shown only for a step whose agent ran on a native connection, from the permissions it ran
//! with.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::super::super::ProjectNav;
use crate::{
  navigation::route::{AgentSelection, Page},
  tasks::detail::{RunView, StepView},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    pressable::surface,
  },
};

/// One card per agent that ran on a native connection with something it could not enforce, in
/// pipeline order.
pub(super) fn not_enforced(ctx: &Ctx, nav: &ProjectNav, run: &RunView) -> Vec<Element> {
  let mut shown: Vec<&str> = Vec::new();
  let mut cards = Vec::new();
  for step in &run.steps {
    let Some(agent) = step.agent.as_deref() else {
      continue;
    };
    if shown.contains(&agent) || !ran_natively(run, step) {
      continue;
    }
    if let Some(card) = card(ctx, nav, step, agent) {
      shown.push(agent);
      cards.push(card);
    }
  }
  cards
}

/// Whether `step`'s agent ran on a native connection in this run.
fn ran_natively(run: &RunView, step: &StepView) -> bool {
  step.provider.is_some_and(|provider| provider.is_native())
    && step
      .execution
      .is_some_and(|execution| run.invocations_of(execution).next().is_some())
}

fn card(ctx: &Ctx, nav: &ProjectNav, step: &StepView, agent: &str) -> Option<Element> {
  let permissions = step.permissions.as_ref()?;
  let mut lines = Vec::new();
  if permissions.network_stays_on {
    let text = ctx.t_args("runs.notEnforced.networkStayedOn", [("agent", agent)]);
    lines.push(line(Icon::Wifi, &text));
  }
  if !permissions.unconfined_commands.is_empty() {
    let commands = permissions.unconfined_commands.join(&*ctx.t("common.listSeparator"));
    let text = ctx.t_args("runs.notEnforced.listedUnconfined", [("commands", commands.as_str())]);
    lines.push(line(Icon::Terminal, &text));
  }
  if lines.is_empty() {
    return None;
  }
  let link = nav
    .link(Page::Agents(AgentSelection::Profile(permissions.profile_id)))
    .on(surface().id("task-not-enforced-profile"))
    .child(
      Text::new(&ctx.t_args("runs.notEnforced.permissionsLink", [("agent", agent)]))
        .variant(TypographyStyle::Link)
        .color(PaletteColor::TextPrimary),
    );
  let copy = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Inline.role())
    .child(
      Text::new(&ctx.t_args("runs.notEnforced.heading", [("agent", agent)]))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextPrimary),
    )
    .with_children(lines)
    .child(Row::new().child(link));
  Some(
    Row::new()
      .id("task-not-enforced")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Start)
      .background(PaletteColor::SurfaceRaised)
      .rounded(ExtraRadius::Notice.role())
      .child(icon(Icon::ShieldAlert, IconSize::Notice, PaletteColor::TextSecondary))
      .child(copy)
      .into(),
  )
}

/// A line of the card (`dPZFf`, `aiiXE`): a subtle 12px glyph and what was not enforced.
fn line(glyph: Icon, text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(text)
        .variant(ExtraTypography::WrappingProperty.role())
        .color(PaletteColor::TextSecondary)
        .flex(1.0),
    )
    .into()
}
