//! Project & access inspector (`i37vu`) and the Access tab (`lfvWC`).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  node::{border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tools::data::{ProjectTools, ToolEntry};

use super::{editor::EditorProps, words};
use crate::{
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::{PropertyValue, link, note, property, section_title},
  },
};

pub(super) fn inspector(ctx: &Ctx, p: &EditorProps) -> Column {
  // Inspector `i37vu`: 340 wide, its header `mVwpH` 44 high.
  const WIDTH: f32 = 340.0;
  const HEADER: f32 = 44.0;
  let kind = p
    .entry
    .as_ref()
    .map_or(ctx.t("tools.command"), |e| ctx.t(words::kind(&e.tool).0));
  let version = p
    .entry
    .as_ref()
    .map_or(ctx.t("tools.notSaved"), |e| words::version(ctx, e));
  let secrets = ctx.t(if p.entry.as_ref().is_some_and(|e| e.tool.access().is_some()) {
    "tools.needsSecret"
  } else {
    "tools.noSecrets"
  });
  let body = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
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
        .unavailable(ctx.t("tools.accessTitle")),
    )
    .child(section_title("tool-used-title", &ctx.t("tools.usedBy"), None))
    .child(uses(ctx, &p.data.listed, p.entry.as_ref()))
    .child(note("tool-pinned", Icon::Pin, &ctx.t("tools.usesNote")))
    .child(Spacer::new().flex(1.0))
    .child(note("tool-command-note", Icon::Info, &ctx.t("tools.commandNote")));
  Column::new()
    .id("tool-inspector")
    .width(Dimension::Px(WIDTH))
    .height(Dimension::Pct(100.0))
    .flex_shrink(0.0)
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(HEADER))
        .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
        .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(Text::new(&ctx.t("tools.inspectorTitle")).variant(TypographyStyle::Body)),
    )
    .child(ScrollVertical::new(body).flex(1.0))
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
