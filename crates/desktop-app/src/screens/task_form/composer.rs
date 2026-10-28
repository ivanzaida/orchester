//! The composer (`Composer` `IxeaX`): one 720 wide column, its parts 20 apart, centred in the scroll area: the head
//! row, a note where there is one, the title and the outcome typed on the page, the acceptance criteria inline, the
//! property chips in two rows (`Properties` `TUAba`: Pipeline, Delivers, Limits, Sources, then Module, Priority, Points
//! and *…*; 8 apart, 4 over them) and, once a pipeline is chosen, the plan line. While a newer version conflicts with
//! the edits, the fields to choose between take the place of everything under the head.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{dimension::Dimension, padding::Padding},
};

use super::{
  FORM_WIDTH,
  brief::{BriefFields, outcome, title},
  chips::chip_row,
  conflict::conflict_parts,
  criteria::criteria,
  delivers::delivers_chip,
  form::{FormProps, TaskForm},
  head::{head, notes},
  limits::{LimitFields, limits_chip},
  merging::more,
  module::module_chip,
  plan::{pipeline_chip, plan_line},
  points::points_chip,
  priority::priority_chip,
  sources::{SourcesProps, sources_chip},
  state::View,
};
use crate::theme::TaskFormSpacing;

impl TaskForm {
  pub(super) fn composer(&self, ctx: &mut Ctx, props: &FormProps, view: &View, separator: &str) -> Column {
    let composer = Column::new()
      .id("task-form")
      .ref_element(self.column.clone())
      .width(Dimension::Px(FORM_WIDTH))
      .spacing(TaskFormSpacing::Composer.role())
      .child(head(ctx, view, &self.nav))
      .with_children(notes(ctx, view));
    if let Some(conflict) = &view.conflict {
      let pipelines = props.pipelines.as_deref().map(Vec::as_slice);
      return composer.with_children(conflict_parts(ctx, conflict, &self.choices, pipelines));
    }
    if view.subject.is_new() && !self.title_focused.get_untracked() {
      self.title_focused.set(true);
      ctx.focus(&self.title_input);
    }
    let brief = BriefFields {
      title: &self.fields.title,
      brief: &self.fields.brief,
      title_input: &self.title_input,
    };
    let project = props.project.open.project.id();
    composer
      .child(title(ctx, view, &brief))
      .child(outcome(ctx, view, &brief))
      .child(criteria(ctx, view, &self.fields.criteria))
      .child(self.properties(ctx, props, view, separator))
      .with_children(plan_line(ctx, view, project))
  }

  /// The chips in their two rows.
  fn properties(&self, ctx: &mut Ctx, props: &FormProps, view: &View, separator: &str) -> Column {
    let chips = &self.chips;
    let fields = &self.fields;
    let limits = LimitFields {
      budget: &fields.budget,
      time_limit: &fields.time_limit,
      defaults: props.limits,
    };
    let sources = SourcesProps {
      repo_root: props.project.open.repo_root.clone(),
      project: props.project.open.project.id(),
      project_name: props.project.open.project.name().as_str().into(),
      task: view.subject.task().map(|task| task.id()),
      pipeline: view.pipeline().map(|choice| (choice.id, choice.version)),
      drafted: self.sources.clone(),
      locked: view.locked(),
      column: self.column.clone(),
    };
    let mut run = vec![pipeline_chip(
      ctx,
      view,
      (&chips.pipeline, &self.nav),
      (&fields.pipeline, view.tried_run),
    )];
    run.push(delivers_chip(
      ctx,
      view,
      (props.shared, &chips.delivers),
      &fields.delivers,
    ));
    run.push(limits_chip(ctx, view, (&chips.limits, &limits), separator));
    run.push(sources_chip(ctx, sources));
    let mut organize: Vec<_> = module_chip(ctx, view, (&props.modules, &chips.module), &fields.module)
      .into_iter()
      .collect();
    organize.push(priority_chip(ctx, view, &chips.priority, &fields.priority));
    organize.push(points_chip(ctx, view, &chips.points, &fields.points));
    organize.extend(more(ctx, view, (props.shared, &chips.more), &fields.no_automatic_merge));
    Column::new()
      .id("task-form-properties")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xs))
      .spacing(SpacingSize::Sm)
      .child(chip_row("task-form-run-chips", run))
      .child(chip_row("task-form-organize-chips", organize))
  }
}
