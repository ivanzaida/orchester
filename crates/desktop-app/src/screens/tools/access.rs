//! A tool's Access tab (`04c` `zp1FL`): the pipeline steps that call it, each with Open pipeline, then the access
//! rules — not applied yet, so one row says what holds today and what is planned, and Edit access rules stays
//! unavailable.
use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::tools::data::ToolUse;

use super::editor::EditorProps;
use crate::{
  navigation::Navigation,
  routes::{PipelinePanel, ProjectPage, Route},
  screens::hub::parts::{Entry, boxed, group, head, note},
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub(super) fn access(ctx: &mut Ctx, p: &EditorProps) -> Column {
  let uses: Vec<_> = p.entry.iter().flat_map(|e| p.data.listed.used_by(e.id)).collect();
  let rows: Vec<Element> = if uses.is_empty() {
    vec![Entry::new("tool-access-none", ctx.t("tools.noUses")).into()]
  } else {
    let navigation = Navigation::of(ctx);
    uses
      .iter()
      .enumerate()
      .map(|(index, used)| caller(ctx, (p, &navigation), index, used))
      .collect()
  };
  let edit = ActionButton::new("tool-access-edit", ButtonKind::Ghost, ctx.t("tools.editAccess"))
    .icon(Icon::SlidersHorizontal)
    .unavailable(ctx.t("tools.locked"));
  let rule = Entry::new("tool-access-rules", ctx.t("tools.accessNow")).hint(ctx.t("tools.accessPlanned"));
  Column::new()
    .id("tool-access-tab")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::Sections.role())
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(lurq::app::theme::SpacingSize::Sm)
        .child(group(
          "tool-access-uses",
          head("tool-access-uses", &ctx.t("tools.usedTitle"), None, None),
          boxed("tool-access-steps", rows),
        ))
        .child(note("tool-uses-note", Icon::Pin, &ctx.t("tools.usesNote"))),
    )
    .child(group(
      "tool-rules",
      head(
        "tool-rules",
        &ctx.t("tools.rulesTitle"),
        Some(&ctx.t("tools.rulesStatus")),
        Some(edit.into()),
      ),
      boxed("tool-rules-rows", [rule.into()]),
    ))
}

/// A step that calls the tool: its pipeline version and step, what decides it, and Open pipeline.
fn caller(ctx: &Ctx, (p, navigation): (&EditorProps, &Navigation), index: usize, used: &ToolUse) -> Element {
  let step = ctx.t_args(
    "tools.stepTitle",
    [
      ("ordinal", format!("{:02}", used.ordinal)),
      ("step", used.step.to_string()),
    ],
  );
  let title = ctx.t_args(
    "tools.use",
    [
      ("pipeline", used.pipeline_name.to_string()),
      ("version", used.version.get().to_string()),
      ("step", step.to_string()),
    ],
  );
  let route = Route::Project(
    p.project.open.project.id(),
    ProjectPage::Pipeline(used.pipeline, PipelinePanel::default()),
  );
  let open = ActionButton::new(
    format!("tool-access-open-{index}"),
    ButtonKind::Ghost,
    ctx.t("tools.openPipeline"),
  )
  .icon(Icon::ArrowUpRight)
  .opens(navigation.to(route));
  Entry::new(format!("tool-access-step-{index}"), title)
    .hint(ctx.t_args("tools.useHint", [("version", used.version.get().to_string())]))
    .end(open)
    .into()
}
