//! The tool editor's right panel (`k97jh`): who owns the tool, its kind, secrets, access and
//! version; editing access rules, which does not exist yet; and the published pipeline steps that
//! call it, each opening its pipeline.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Spacer, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::ToolKind;

use super::{Editing, EditorContext, editor::EditorState, row::kind_key};
use crate::{
  agents::data::VersionState,
  navigation::route::Page,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{button::ActionButton, icon::Icon, property::property},
};

pub(super) fn inspector(ctx: &Ctx, context: &EditorContext, state: &EditorState) -> Element {
  let (kind, access, version, uses) = match &context.editing {
    Editing::New => (ToolKind::Command, false, None, Vec::new()),
    Editing::Tool(entry) => (
      entry.tool.kind(),
      entry.tool.access().is_some(),
      Some(entry.state),
      context.tools.used_by(entry.id).cloned().collect(),
    ),
  };
  let secrets = if kind == ToolKind::Command {
    ctx.t("tools.secrets.none")
  } else {
    ctx.t("tools.secrets.notAvailable")
  };
  let access = if access {
    ctx.t("tools.inspector.accessRules")
  } else {
    ctx.t("tools.inspector.accessSteps")
  };
  let properties = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Property.role())
    .child(property(
      &ctx.t("tools.inspector.owner"),
      context.project.project.name().as_str(),
    ))
    .child(property(&ctx.t("tools.inspector.kind"), &ctx.t(kind_key(kind))))
    .child(property(&ctx.t("tools.inspector.secrets"), &secrets))
    .child(property(&ctx.t("tools.inspector.access"), &access))
    .child(property(
      &ctx.t("tools.inspector.version"),
      &version_value(ctx, version, state.dirty),
    ));
  Column::new()
    .id("tool-inspector")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(SpacingSize::Lg)
    .spacing(ExtraSpacing::Form.role())
    .background(PaletteColor::SurfacePanel)
    .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    // The full-width buttons' focus rings reach past the column.
    .overflow_visible()
    .child(Text::new(&ctx.t("tools.inspector.title")).variant(ExtraTypography::CardTitle.role()))
    .child(properties)
    .child(
      ActionButton::new(ctx.t("tools.inspector.editAccess"))
        .icon(Icon::SlidersHorizontal)
        .id("tool-edit-access")
        .fill_width()
        .render(ctx),
    )
    .child(used_by(ctx, context, &uses))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&ctx.t(if kind == ToolKind::Command {
        "tools.inspector.note"
      } else {
        "tools.inspector.noteSecrets"
      }))
      .variant(ExtraTypography::Detail.role()),
    )
    .into()
}

/// "v2 draft", "v1", "Not published".
fn version_value(ctx: &Ctx, version: Option<VersionState>, dirty: bool) -> Arc<str> {
  let next = |published: Option<orchester_core::resource::VersionNumber>| {
    published.map_or(1, |version| version.get() + 1).to_string()
  };
  match version {
    None => ctx.t("tools.notPublished"),
    Some(VersionState::Published(version)) if !dirty => {
      ctx.t_args("tools.version", [("version", version.get().to_string().as_str())])
    }
    Some(version) => ctx.t_args(
      "tools.inspector.draftVersion",
      [("version", next(version.published()).as_str())],
    ),
  }
}

/// The published pipeline steps that call the tool (`zZmQ0`).
fn used_by(ctx: &Ctx, context: &EditorContext, uses: &[crate::tools::data::ToolUse]) -> Element {
  let mut column = Column::new()
    .id("tool-used-by")
    .width(Dimension::Pct(100.0))
    .padding_top(ExtraSpacing::Step.role())
    .spacing(ExtraSpacing::Group.role())
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .overflow_visible()
    .child(Text::new(&ctx.t("tools.inspector.usedBy")).variant(ExtraTypography::FormLabel.role()));
  if uses.is_empty() {
    column = column.child(Text::new(&ctx.t("tools.inspector.unused")).variant(TypographyStyle::Caption));
  }
  for used in uses {
    let label = ctx.t_args(
      "tools.inspector.use",
      [("pipeline", &*used.pipeline_name), ("step", &*used.step)],
    );
    let (nav, pipeline) = (context.nav.clone(), used.pipeline);
    column = column.child(
      ActionButton::new(label)
        .icon(Icon::Workflow)
        .compact()
        .start()
        .fill_width()
        .on_click(move || nav.show(Page::Pipeline(pipeline)))
        .render(ctx),
    );
  }
  column
    .child(Text::new(&ctx.t("tools.inspector.versions")).variant(ExtraTypography::Detail.role()))
    .into()
}
