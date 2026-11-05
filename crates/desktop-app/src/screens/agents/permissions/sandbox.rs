//! *Sandbox* (ORC-789; not drawn yet, it follows *Network* `I9OrmW` in 07): a Codex connection's sandbox as a switch,
//! on by default, warning while off that Codex then enforces neither repository access, network nor writable folders.
//! Only for a Codex connection of a local project: a shared project's profiles keep it on in this version, and other
//! CLIs have no such sandbox. Turning it off widens, so it is asked about here like the other sections' widenings.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::AgentProvider, permission::CodexSandbox};

use super::{super::fields::Fields, Context, section};
use crate::{
  screens::hub::{parts::field, team},
  theme::{HubSpacing, HubText},
  ui::{
    icon::Icon,
    tinted_note::tinted_note,
    toggle::{Toggle, ToggleKind},
  },
};

/// Whether the page offers the switch: on a Codex connection of a project that is not shared.
pub(super) fn offered(context: &Context<'_>, provider: Option<AgentProvider>) -> bool {
  provider == Some(AgentProvider::CodexCli) && team::of(context.project).is_none()
}

/// The section: *Codex sandbox* with its state and switch, the warning under it while off, and `asked` last.
pub(super) fn sandbox(ctx: &Ctx, fields: &Fields, asked: Option<Element>) -> Element {
  let on = fields.permissions.get().0.codex_sandbox().is_on();
  let value = fields.permissions.clone();
  let state = ctx.t(if on { "agents.on" } else { "agents.off" });
  let toggle = Toggle::new("agent-codex-sandbox", ToggleKind::Switch, on, state.clone())
    .hide_label()
    .on_change(move |on| {
      let sandbox = if on { CodexSandbox::On } else { CodexSandbox::Off };
      value.update(|p| p.0.set_codex_sandbox(sandbox));
    });
  let control = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&state).variant(HubText::Status.role()))
    .child(toggle);
  let row = field(
    "agent-codex-sandbox",
    &ctx.t("agents.codexSandbox"),
    Some(&ctx.t("agents.codexSandboxHint")),
    control,
  );
  let switch = Column::new()
    .width(Dimension::Pct(100.0))
    .child(row)
    .with_children((!on).then(|| {
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(
          Padding::new()
            .left(HubSpacing::Row.role())
            .right(HubSpacing::Row.role())
            .bottom(HubSpacing::FieldY.role()),
        )
        .child(tinted_note(
          "agent-codex-sandbox-warning",
          (Icon::TriangleAlert, PaletteColor::Warning),
          &ctx.t("agents.codexSandboxOff"),
        ))
    }));
  let mut rows = vec![switch.into()];
  rows.extend(asked);
  section("agent-sandbox", (&ctx.t("agents.sandbox"), None), rows)
}
