//! The groups of the Sources popover (`This task` `QGbtc`, `From the project` `gkkmJ`, `From the pipeline` `QF70r`): a
//! group's head, its name in 11/500 and its count in 11 `text_muted` with where it is changed at its end in 11
//! `text_secondary`; the rows of what the task gets from the project and from its pipeline, read-only with a lock in
//! place of the cross; and how many sources a run of the task starts with (`Summary` `atj72`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::sources::{
  check::Kind,
  listed::Listed,
  task::{FromPipeline, Inherited},
};

use super::SourcesProps;
use crate::{
  navigation::Navigation,
  routes::{PipelineView, ProjectPage, ProjectSettingsTab, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    source_row::SourceRow,
  },
};

/// A group's head: its name, how many it holds, and where it is changed.
pub(super) fn subhead(ctx: &Ctx, id: &str, key: &str, count: usize, link: Option<Element>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .child(
      Text::new(&ctx.t_args("task_form.sources.count", [("count", count.to_string())]))
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Caption),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(link)
}

/// A link at a head's end (`Link` `WilsF`): 11 `text_secondary` and an `arrow-up-right`.
fn link(ctx: &mut Ctx, id: &str, key: &str, route: Route) -> Element {
  let open = Navigation::of(ctx).to(route);
  opening(open)
    .id(id.to_owned())
    .child(
      Row::new()
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t(key))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::TextSecondary),
        )
        .child(icon(Icon::ArrowUpRight, IconSize::Xs, PaletteColor::TextSecondary)),
    )
    .into()
}

/// What the task gets from the project and, once a pipeline is chosen, from its pipeline, read-only.
pub(super) fn inherited(ctx: &mut Ctx, props: &SourcesProps, inherited: &Inherited) -> Vec<Element> {
  let settings = Route::Project(props.project, ProjectPage::Settings(ProjectSettingsTab::Context));
  let edit = link(
    ctx,
    "task-form-sources-settings",
    "task_form.sources.editInSettings",
    settings,
  );
  let mut project = group(subhead(
    ctx,
    "task-form-sources-project",
    "task_form.sources.fromProject",
    inherited.project.len(),
    Some(edit),
  ));
  for (index, listed) in inherited.project.iter().enumerate() {
    let origin = props.project_name.to_string();
    project = project.child(locked_row(
      ctx,
      &format!("task-form-project-source-{index}"),
      listed,
      &origin,
    ));
  }
  let mut groups = vec![project.into()];
  let Some((pipeline, _)) = props.pipeline else {
    return groups;
  };
  let route = Route::Project(props.project, ProjectPage::Pipeline(pipeline, PipelineView::Steps));
  let view = link(
    ctx,
    "task-form-sources-pipeline-link",
    "task_form.sources.viewPipeline",
    route,
  );
  let mut from_pipeline = group(subhead(
    ctx,
    "task-form-sources-pipeline",
    "task_form.sources.fromPipeline",
    inherited.pipeline.len(),
    Some(view),
  ));
  for (index, from) in inherited.pipeline.iter().enumerate() {
    let origin = pipeline_origin(ctx, from);
    from_pipeline = from_pipeline.child(locked_row(
      ctx,
      &format!("task-form-pipeline-source-{index}"),
      &from.listed,
      &origin,
    ));
  }
  groups.push(from_pipeline.into());
  groups
}

/// A group: its head and its rows, 2 apart.
pub(super) fn group(head: Row) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(head)
}

fn pipeline_origin(ctx: &Ctx, from: &FromPipeline) -> String {
  message_text(ctx, &from.origin()).to_string()
}

/// A source listed elsewhere: where, and a lock in place of the cross.
fn locked_row(ctx: &mut Ctx, id: &str, listed: &Listed, origin: &str) -> Element {
  let meta = match listed.kind {
    Some(Kind::Folder) => ctx.t_args("task_form.sources.folderOf", [("origin", origin)]),
    Some(Kind::File) => ctx.t_args("task_form.sources.fileOf", [("origin", origin)]),
    None => ctx.t_args("task_form.sources.goneOf", [("origin", origin)]),
  };
  let mut row = SourceRow::new(id, listed.path.as_str(), listed.kind, meta)
    .status(listed.state)
    .locked();
  if let Some(problem) = listed.problem() {
    row = row.note(message_text(ctx, &problem));
  }
  row.render(ctx)
}

/// How many sources a run of the task starts with (`Summary` `atj72`).
pub(super) fn summary(ctx: &Ctx, count: usize) -> Element {
  let words = match count {
    0 => ctx.t("task_form.sources.summaryNone"),
    1 => ctx.t("task_form.sources.summaryOne"),
    count => ctx.t_args("task_form.sources.summary", [("count", count.to_string())]),
  };
  Text::new(&words)
    .id("task-form-sources-summary")
    .width(Dimension::Pct(100.0))
    .variant(ExtraTypography::Note.role())
    .into()
}
