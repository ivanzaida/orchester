//! Project & access beside a tool's page (`04b`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  node::dimension::Dimension,
};
use orchester_workspace::tools::data::{ProjectTools, ToolEntry};

use super::{editor::EditorProps, words};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::{PropertyValue, link, note, property, section, section_title},
  },
};

/// The side beside a tool's page (`Project & access` in 04b): who owns it, its kind, secrets, access and version,
/// the steps that call it, and what a command tool holds.
pub(super) fn inspector(ctx: &Ctx, p: &EditorProps) -> Column {
  let count = p.entry.iter().flat_map(|e| p.data.listed.used_by(e.id)).count();
  let bottom = ctx.t(if p.entry.as_ref().is_some_and(|e| !e.runnable()) {
    "tools.secretsNote"
  } else {
    "tools.commandNote"
  });
  Column::new()
    .id("tool-inspector")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(Text::new(&ctx.t("tools.inspectorTitle")).variant(crate::theme::HubText::GroupTitle.role()))
    .child(properties(ctx, p))
    .child(crate::ui::unavailable::reason_line(
      "tool-inspector-access-reason",
      &ctx.t("tools.accessTitle"),
    ))
    .child(
      section("tool-used-section", SpacingSize::Xs)
        .child(section_title(
          "tool-used-title",
          &ctx.t("tools.usedBy"),
          Some(&ctx.t_args("tools.stepsCount", [("count", count.to_string())])),
        ))
        .child(uses(ctx, &p.data.listed, p.entry.as_ref()))
        .child(note("tool-pinned", Icon::Pin, &ctx.t("tools.pinnedNote"))),
    )
    .child(note("tool-command-note", Icon::Info, &bottom))
}
fn uses(ctx: &Ctx, listed: &ProjectTools, entry: Option<&ToolEntry>) -> Column {
  let uses: Vec<_> = entry.into_iter().flat_map(|e| listed.used_by(e.id)).collect();
  let mut column = Column::new()
    .id("tool-uses")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs);
  if uses.is_empty() {
    return column.child(Text::new(&ctx.t("tools.noUses")).variant(TypographyStyle::Caption));
  }
  for (i, used) in uses.into_iter().enumerate() {
    let title = ctx.t_args(
      "tools.use",
      [
        ("pipeline", used.pipeline_name.to_string()),
        ("version", used.version.get().to_string()),
        ("step", used.step.to_string()),
      ],
    );
    // Pipeline editing is not built yet: the link names the real step without pretending to open it.
    column = column.child(link(&format!("tool-use-{i}"), Icon::Workflow, &title, None));
  }
  column
}
fn properties(ctx: &Ctx, p: &EditorProps) -> Column {
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
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(property(
      "tool-owner",
      &ctx.t("tools.owner"),
      p.project.open.project.name().as_str(),
      PropertyValue::Plain,
    ))
    .child(property("tool-kind", &ctx.t("tools.kind"), &kind, PropertyValue::Plain))
    .child(property(
      "tool-secrets",
      &ctx.t("tools.secrets"),
      &secrets,
      PropertyValue::Muted,
    ))
    .child(property(
      "tool-access",
      &ctx.t("tools.access"),
      &ctx.t("tools.accessValue"),
      PropertyValue::Plain,
    ))
    .child(property(
      "tool-version",
      &ctx.t("tools.version"),
      &version,
      PropertyValue::Plain,
    ))
    .child(
      ActionButton::new("tool-edit-access", ButtonKind::Secondary, ctx.t("tools.editAccess"))
        .icon(Icon::SlidersHorizontal)
        .fill_width()
        .disabled(),
    )
}
