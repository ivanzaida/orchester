//! The Connection & model section as a member reads a shared profile on this computer: the team's choices in value
//! boxes (06 `sakAV`, 06c `FO7Cq`, 04 `w0Evp`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{
  super::{entries, fields::Fields, shared, shared::ConnectionHere, words},
  CONTROL_WIDTH,
};
use crate::{
  screens::hub::parts::{boxed, field, group, head, note},
  theme::{ControlHeight, ExtraSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// The Connection & model section as a member reads a shared profile on this computer. An agent CLI's profile names
/// the CLI, the model and the effort the team chose, then the member's own *Runs here use* with `note_under` under the
/// section (06 `sakAV`, 06c `FO7Cq`); an API's names the project connection, *Missing on this computer* while nothing
/// here is bound to it, with the assignment note (04 `w0Evp`).
pub(in crate::screens::agents) fn connection_shared(
  ctx: &mut Ctx,
  fields: &Fields,
  here: &ConnectionHere,
  (runs_here, note_under): (Option<Element>, Option<Element>),
) -> Element {
  let mut rows = shared_rows(ctx, fields, here);
  rows.extend(runs_here);
  let note_under =
    note_under.unwrap_or_else(|| note("agent-assignment-note", Icon::Info, &ctx.t("agents.assignmentNote")).into());
  Column::new()
    .id("agent-model-section")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(group(
      "agent-model-group",
      head("agent-model-group", &ctx.t("agents.connectionModel"), None, None),
      boxed("agent-model-rows", rows),
    ))
    .child(note_under)
    .into()
}

/// The rows the team chose: the agent CLI or the project connection, the model and the effort.
fn shared_rows(ctx: &Ctx, fields: &Fields, here: &ConnectionHere) -> Vec<Element> {
  let effort = fields.effort.get();
  let effort = if effort.is_empty() {
    ctx.t("agents.defaultEffort")
  } else {
    words::effort(ctx, &effort)
  };
  let (first, (model_hint, effort_hint)) = match here.cli {
    Some(cli) => {
      let name = shared::cli_name(ctx, cli);
      let version = here.version.map_or(0, |version| version.get());
      let hint = ctx.t_args(
        "sharing.member.cliHint",
        [("profile", fields.role.get()), ("version", version.to_string())],
      );
      let first = field(
        "agent-cli",
        &ctx.t("sharing.member.cli"),
        Some(&hint),
        value_box("agent-cli-value", (Some(entries::cli_glyph(cli)), &name), Look::Plain),
      );
      let model_hint = ctx.t_args("sharing.member.modelHint", [("cli", name.as_ref())]);
      (first, (model_hint, ctx.t("sharing.member.effortHint")))
    }
    None => {
      let (connection, look) = here.local.as_ref().map_or_else(
        || (ctx.t("sharing.runsHere.missing"), Look::Missing),
        |local| (local.as_str().into(), Look::Plain),
      );
      let hint = ctx.t_args(
        "sharing.member.apiHint",
        [("name", here.name.as_str()), ("role", here.role.as_str())],
      );
      let first = field(
        "agent-connection",
        &ctx.t("agents.connection"),
        Some(&hint),
        value_box("agent-connection-value", (None, &connection), look),
      );
      (first, (ctx.t("agents.modelHint"), ctx.t("agents.effortHint")))
    }
  };
  vec![
    first.into(),
    field(
      "agent-model",
      &ctx.t("agents.model"),
      Some(&model_hint),
      value_box("agent-model-value", (None, &fields.model_value()), Look::Code),
    )
    .into(),
    field(
      "agent-effort",
      &ctx.t("agents.effort"),
      Some(&effort_hint),
      value_box("agent-effort-value", (None, &effort), Look::Plain),
    )
    .into(),
  ]
}

/// How a value box words its value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
  Plain,
  /// A model id, in mono.
  Code,
  /// Nothing here is bound to it: *Missing on this computer* in `danger` (`YcxOd` in 04).
  Missing,
}

/// A value the team chose, in a box where a member's control would go, with its CLI's mark before it (`Value box`
/// `rYGpH` in 06c).
fn value_box(id: &str, (glyph, text): (Option<Icon>, &str), look: Look) -> Element {
  let words = Text::new(text)
    .id(id.to_owned())
    .variant(if look == Look::Code {
      HubText::Area.role()
    } else {
      HubText::Value.role()
    })
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  Row::new()
    .id(format!("{id}-box"))
    .width(Dimension::Px(CONTROL_WIDTH))
    .height(Dimension::Px(ControlHeight::Row.value()))
    .flex_shrink(0.0)
    .padding(
      Padding::new()
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Md, PaletteColor::TextPrimary)))
    .child(if look == Look::Missing {
      words.color(PaletteColor::Danger)
    } else {
      words
    })
    .into()
}
