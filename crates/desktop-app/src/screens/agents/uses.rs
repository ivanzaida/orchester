//! Used in pipelines uIMpt: drafts, published versions, overrides, and pinned active runs.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::agents::usage::PipelineUse;

use super::{editor::Props, words};
use crate::ui::table::{ColumnWidth, table_header};

pub fn render(ctx: &mut Ctx, props: &Props) -> Element {
  let profile = props.entry.as_ref().map(|e| e.id);
  let steps: Vec<_> = props
    .listed
    .usage
    .steps
    .iter()
    .filter(|s| Some(s.profile) == profile)
    .collect();
  let mut page = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(words::note(&ctx.t("agents.usesNote")));
  if steps.is_empty() {
    page = page.child(words::note(&ctx.t("agents.noUses")));
  } else {
    let headings = [
      "agents.pipeline",
      "agents.step",
      "agents.usedModel",
      "agents.usePermissions",
      "agents.useRuns",
    ]
    .map(|key| ctx.t(key));
    let mut table = Column::new()
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Lg)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(table_header(
        "agent-uses-header",
        &headings
          .iter()
          .map(|h| (ColumnWidth::Fill, Some(&**h)))
          .collect::<Vec<_>>(),
        false,
      ));
    for (index, step) in steps.iter().enumerate() {
      table = table.child(use_row(ctx, index, step));
    }
    let counts = ctx.t_args(
      "agents.useCounts",
      [
        (
          "pipelines",
          profile.map_or(0, |id| props.listed.usage.count(id)).to_string(),
        ),
        ("steps", steps.len().to_string()),
        (
          "overrides",
          steps.iter().filter(|s| s.model_overridden).count().to_string(),
        ),
      ],
    );
    page = page.child(table.child(Row::new().padding(SpacingSize::Md).child(words::note(&counts))));
  }
  page.child(active(ctx, props)).into()
}
fn use_row(ctx: &Ctx, index: usize, step: &PipelineUse) -> Element {
  let version = ctx.t_args(
    if step.draft {
      "agents.draftVersion"
    } else {
      "agents.published"
    },
    [("version", step.version.to_string())],
  );
  let model = step
    .model
    .as_ref()
    .map_or_else(|| ctx.t("agents.chooseConnection"), |m| m.as_str().into());
  let state = if step.draft {
    ctx.t("agents.draftNoRuns")
  } else if step.running == 0 {
    ctx.t("agents.noRuns")
  } else {
    ctx.t_args("agents.runningCount", [("count", step.running.to_string())])
  };
  Row::new()
    .id(format!("agent-use-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(64.0))
    .padding(SpacingSize::Md)
    .spacing(SpacingSize::Md)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Column::new()
        .flex(1.0)
        .child(words::note(&step.name))
        .child(words::note(&version)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .child(words::note(&format!("{:02} {}", step.position, step.step))),
    )
    .child(
      Column::new()
        .flex(1.0)
        .child(words::note(&model))
        .child(words::note(&ctx.t(if step.model_overridden {
          "agents.override"
        } else {
          "agents.fromProfile"
        }))),
    )
    .child(Column::new().flex(1.0).child(words::note(&ctx.t(if step.narrowed {
      "agents.narrowed"
    } else {
      "agents.samePermissions"
    }))))
    .child(Column::new().flex(1.0).child(words::note(&state)))
    .into()
}
fn active(ctx: &mut Ctx, props: &Props) -> Element {
  let mut section = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(Text::new(&ctx.t("agents.activeTitle")).variant(TypographyStyle::Title));
  let active: Vec<_> = props
    .listed
    .usage
    .active
    .iter()
    .filter(|r| props.entry.as_ref().is_some_and(|p| p.id == r.profile))
    .collect();
  if active.is_empty() {
    return section.child(words::note(&ctx.t("agents.noActive"))).into();
  }
  let navigation = crate::navigation::Navigation::of(ctx);
  for (index, run) in active.iter().enumerate() {
    let nav = navigation.clone();
    let route = crate::routes::Route::Project(
      props.project.open.project.id(),
      crate::routes::ProjectPage::Task(run.task.clone(), crate::routes::TaskView::Summary),
    );
    let title = ctx.t_args(
      "agents.activeTask",
      [("key", run.task.to_string()), ("title", run.title.to_string())],
    );
    let detail = ctx.t_args(
      "agents.activeWhere",
      [
        ("pipeline", run.pipeline.to_string()),
        ("version", run.pipeline_version.get().to_string()),
        ("step", run.step.to_string()),
      ],
    );
    section = section.child(
      crate::ui::pressable::pressable(move || nav.go(route.clone()))
        .id(format!("agent-active-{index}"))
        .width(Dimension::Pct(100.0))
        .child(
          Row::new()
            .width(Dimension::Pct(100.0))
            .padding(SpacingSize::Md)
            .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
            .rounded(RadiusSize::Lg)
            .child(
              Column::new()
                .flex(1.0)
                .child(words::note(&title))
                .child(words::note(&detail)),
            )
            .child(words::note(
              &ctx.t_args("agents.keepsVersion", [("version", run.version.get().to_string())]),
            )),
        ),
    );
  }
  section.into()
}
