//! Project & access beside a tool's page (`Side` `wfral` in `04b` `nuoSy`): who owns it, its kind, secrets, access
//! and version in one box, then what a command tool holds. The steps that call it are on the Access tab.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};

use super::{editor::EditorProps, words};
use crate::{
  screens::hub::parts::{Entry, boxed, group, head, note},
  theme::HubText,
  ui::icon::Icon,
};

pub(super) fn inspector(ctx: &Ctx, p: &EditorProps) -> Column {
  let bottom = ctx.t(if p.entry.as_ref().is_some_and(|e| !e.runnable()) {
    "tools.secretsNote"
  } else {
    "tools.commandNote"
  });
  Column::new()
    .id("tool-inspector")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(group(
      "tool-project-access",
      head("tool-project-access", &ctx.t("tools.inspectorTitle"), None, None),
      boxed("tool-properties", properties(ctx, p)),
    ))
    .child(note("tool-command-note", Icon::Info, &bottom))
}

fn properties(ctx: &Ctx, p: &EditorProps) -> Vec<Element> {
  let kind = p
    .entry
    .as_ref()
    .map_or(ctx.t("tools.command"), |e| ctx.t(words::kind(&e.tool).0));
  let version = p
    .entry
    .as_ref()
    .map_or(ctx.t("tools.notSaved"), |e| words::inspector_version(ctx, e));
  let secrets = ctx.t(if p.entry.as_ref().is_some_and(|e| e.tool.access().is_some()) {
    "tools.needsSecret"
  } else {
    "tools.noSecrets"
  });
  let owner = p.project.open.project.name().to_string();
  [
    ("tool-owner", "tools.owner", owner.as_str().into()),
    ("tool-kind", "tools.kind", kind),
    ("tool-secrets", "tools.secrets", secrets),
    ("tool-access", "tools.access", ctx.t("tools.accessValue")),
    ("tool-version", "tools.version", version),
  ]
  .into_iter()
  .map(|(id, label, value): (&str, &str, std::sync::Arc<str>)| {
    Entry::new(format!("{id}-row"), ctx.t(label))
      .end(
        Text::new(&value)
          .id(id.to_owned())
          .variant(HubText::Value.role())
          .color(lurq::app::theme::PaletteColor::TextSecondary)
          .nowrap(),
      )
      .into()
  })
  .collect()
}
