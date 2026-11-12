//! The *Step* and *Run* groups of an open step's fields (`Step` `wFS9r`, `Run` `ddwhj` in P2 `u2rYEH`), as
//! boxes of rows like the panel's *Pipeline* ([`crate::screens::team_hub::rows`]). Every field changes the edit the
//! panel holds ([`crate::screens::pipeline_hub::edit`]).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{PipelineEdit, Step, StepKey};
use orchester_workspace::pipelines::fields::{Executor, RouteChoice};

use super::{
  Open, agent, choices, executor, keep,
  pieces::Chip,
  text_field::{self, Kind},
};
use crate::{
  screens::{
    pipeline_hub::{
      flow::number,
      pick::{self, Pick},
    },
    team_hub::rows::{self, Sub},
  },
  theme::{ExtraSpacing, HubText},
};

/// *Step*: *Agent* and *Instructions*; a tool that cannot run here says why in place of *Instructions*.
pub(super) fn step(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Element {
  let mut rows = vec![agent_row(ctx, open, step)];
  rows.push(match executor::unavailable_tool(open, step) {
    Some(tool) => executor::tool_notice(ctx, tool),
    None => instructions_row(ctx, open, step),
  });
  rows::section(
    "pipeline-hub-step-group",
    &ctx.t("pipeline_hub.step.group"),
    Vec::new(),
    rows,
  )
  .into()
}

/// *Agent*: the field, and under its box what a choice dropped. Its label stays on the line of the box, so a note under
/// the box moves nothing beside it.
fn agent_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.step.agent");
  let choices = choices::choices(ctx, open.assignables, step);
  let started = open.edit.started_step(&step.key).map(|started| {
    let cli = match Executor::of(&started) {
      Executor::Profile(id) => open.assignables.profile(id).and_then(|profile| profile.provider),
      Executor::Tool(_) | Executor::Unset => None,
    };
    (started, cli)
  });
  let dropped = agent::dropped_note(ctx, started.as_ref(), step);
  let value = agent::field(
    ctx,
    agent::Props {
      edit: open.edit.clone(),
      project: open.project.open.project.id(),
      key: step.key.clone(),
      value: Executor::of(step),
      flag: choices.flag,
      choices: choices.items.into(),
      executors: choices.executors.into(),
      clis: choices.clis.into(),
      cli: choices.cli,
      started,
      dropped,
      name: label.clone(),
      unset: ctx.t("pipeline_hub.agent.choose"),
      locked: open.locked,
      epoch: open.epoch,
      pick: open.pick_agent.clone(),
      paths: open.known.clone(),
    },
  );
  let id = "pipeline-hub-step-agent-row";
  let label = Text::new(&label)
    .id(format!("{id}-label"))
    .variant(HubText::Status.role());
  let key = executor::key(ctx, open, step, label);
  let value: Element = match key.lead {
    Some(lead) => Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(lead.role()))
      .child(value)
      .into(),
    None => value,
  };
  rows::keyed(id, key.label, key.sub, value).align_items(Alignment::Start)
}

/// *Instructions*: a multi-line box edited in place, with what `{{ }}` does under its label.
fn instructions_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.step.instructions");
  let field = text_field::field(
    ctx,
    text_field::Props {
      edit: open.edit.clone(),
      key: step.key.clone(),
      kind: Kind::Instructions,
      shown: Kind::Instructions.shown(step),
      label: label.to_string(),
      placeholder: None,
      unit: None,
      locked: open.locked,
      epoch: open.epoch,
      faulty: open.faulty.clone(),
    },
  );
  // The label stays at the top of a box that grows.
  rows::row(
    "pipeline-hub-step-instructions-row",
    &label,
    Some(Sub::quiet(ctx.t("pipeline_hub.step.instructionsHint"))),
    field,
  )
  .align_items(Alignment::Start)
}

/// *Run*: where changes requested go (with its retries), and the step budget.
pub(super) fn run(ctx: &mut Ctx, open: &Open<'_>, step: &Step, upstream: &[Chip]) -> Element {
  let mut rows = vec![changes_row(ctx, open, step, upstream)];
  if step.on_failure.is_some() {
    rows.push(text_row(ctx, open, step, Kind::Retries));
  } else {
    // Retries that were typed wrong belonged to a route the step no longer has: they are not kept for the next one.
    keep(open.faulty, (&step.key, Kind::Retries), None);
  }
  rows.push(text_row(ctx, open, step, Kind::Budget));
  rows.push(stopped_row(ctx));
  rows::section(
    "pipeline-hub-step-run",
    &ctx.t("pipeline_hub.step.run"),
    Vec::new(),
    rows,
  )
  .into()
}

/// *When changes are requested*: *Fail the run*, or the earlier step the run goes back to.
fn changes_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step, upstream: &[Chip]) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.step.changes");
  let target = step.on_failure.as_ref().map(|route| route.return_to.clone());
  let mut options = vec![(String::new(), ctx.t("pipeline_hub.step.failRun").to_string())];
  options.extend(upstream.iter().filter_map(|chip| {
    let key = chip.key.as_ref()?;
    Some((
      key.as_str().to_owned(),
      format!("{} {}", number(chip.number), chip.name),
    ))
  }));
  // A route that no longer goes to an earlier step is shown as it is, so it can be seen and changed.
  if let Some(target) = target
    .as_ref()
    .filter(|key| options.iter().all(|(value, _)| value != key.as_str()))
  {
    options.push((target.as_str().to_owned(), named(open.current, target)));
  }
  let (edit, key) = (open.edit.clone(), step.key.clone());
  let on_pick: Pick = Arc::new(move |picked| {
    // The select's values are ours: empty is *Fail the run* and any other is the key of a step it listed, so a value
    // that is not a key can only be the empty one, and losing the parse error is right.
    let choice = RouteChoice(StepKey::new(&picked).ok());
    edit.change(|current| {
      current.update(&key, |step| choice.apply(step));
    });
  });
  let select = pick::pick(
    ctx,
    pick::Props {
      id: format!("pipeline-hub-step-changes-{}-{}", step.key, open.epoch),
      name: label.to_string(),
      value: target.map_or_else(String::new, |key| key.as_str().to_owned()),
      options,
      placeholder: None,
      mono: false,
      locked: open.locked,
      choosing: false,
      on_pick,
    },
  );
  rows::row("pipeline-hub-step-changes-row", &label, None, select)
}

/// The number and name of step `key`, as the menu says it.
fn named(edit: &PipelineEdit, key: &StepKey) -> String {
  let at = edit.steps().iter().position(|step| step.key == *key);
  let name = edit.step(key).map_or("", |step| step.name.as_str());
  format!("{} {name}", number(at.map_or(0, |at| at + 1)))
}

/// *Step budget* or *Retries*.
fn text_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step, kind: Kind) -> lurq::components::Row {
  let (id, label, placeholder, unit) = match kind {
    Kind::Budget => (
      "pipeline-hub-step-budget-row",
      ctx.t("pipeline_hub.step.budget"),
      Some(ctx.t("pipeline_hub.step.noLimit").to_string()),
      Some(ctx.t("pipeline_hub.step.tokens").to_string()),
    ),
    Kind::Retries | Kind::Instructions | Kind::Name => (
      "pipeline-hub-step-retries-row",
      ctx.t("pipeline_hub.step.retries"),
      None,
      None,
    ),
  };
  let field = text_field::field(
    ctx,
    text_field::Props {
      edit: open.edit.clone(),
      key: step.key.clone(),
      kind,
      shown: kind.shown(step),
      label: label.to_string(),
      placeholder,
      unit,
      locked: open.locked,
      epoch: open.epoch,
      faulty: open.faulty.clone(),
    },
  );
  rows::row(id, &label, None, field)
}

/// *When the step fails* (`When the step fails` `dvZ8g`): what every step does, said as words where the boxes' words
/// start; a step does not choose it.
fn stopped_row(ctx: &Ctx) -> lurq::components::Row {
  let value = Row::new()
    .flex(1.0)
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .child(
      Text::new(&ctx.t("pipeline_hub.settings.failureValue"))
        .id("pipeline-hub-step-stopped")
        .variant(HubText::Value.role())
        .color(PaletteColor::TextSecondary),
    );
  rows::row(
    "pipeline-hub-step-stopped-row",
    &ctx.t("pipeline_hub.stepEdit.stopped"),
    None,
    value,
  )
}
