//! The groups of an open step's fields (`Step` `wFS9r`, `Run` `ddwhj`, `Gate and access` `hX7lu` in P2 `u2rYEH`), as
//! boxes of rows like the panel's *Pipeline* ([`crate::screens::team_hub::rows`]). Every field changes the edit the
//! panel holds ([`crate::screens::pipeline_hub::edit`]).

use std::sync::Arc;

use lurq::{app::ctx::Ctx, layout::Alignment, node::Element};
use orchester_core::resource::{PipelineEdit, RepositoryAccess, Step, StepKey, StepKind};
use orchester_workspace::pipelines::{
  fields::{AccessChoice, Executor, RouteChoice},
  narrowing,
  sources::Listing,
};

use super::{
  Open, agent, choices, keep,
  pieces::Chip,
  text_field::{self, Kind},
};
use crate::{
  screens::{
    local_hub::profile::pick::{self, Pick},
    pipeline_hub::{
      flow::number,
      sources::{self, Props as SourcesProps},
    },
    team_hub::rows::{self, Sub},
  },
  ui::toggle::{Toggle, ToggleKind},
};

/// *Step*: *Agent* and *Instructions*.
pub(super) fn step(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Element {
  let rows = vec![agent_row(ctx, open, step), instructions_row(ctx, open, step)];
  rows::section(
    "pipeline-hub-step-group",
    &ctx.t("pipeline_hub.step.group"),
    Vec::new(),
    rows,
  )
  .into()
}

fn agent_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.step.agent");
  let choices = choices::choices(ctx, open.assignables, step);
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
      name: label.clone(),
      unset: ctx.t("pipeline_hub.agent.choose"),
      locked: open.locked,
      epoch: open.epoch,
    },
  );
  rows::row("pipeline-hub-step-agent-row", &label, None, value)
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
    Kind::Retries | Kind::Instructions => (
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

/// *Gate and access*: *Wait for your approval*, an agent step's *Repository access* and its *Sources*.
pub(super) fn gate(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> Element {
  let mut rows = vec![approval_row(ctx, open, step)];
  if let StepKind::Agent {
    profile, repository, ..
  } = &step.kind
  {
    rows.push(access_row(ctx, open, step, (*profile, *repository)));
  }
  rows.push(sources_row(ctx, open, step));
  rows::section(
    "pipeline-hub-step-gate",
    &ctx.t("pipeline_hub.step.gateGroup"),
    Vec::new(),
    rows,
  )
  .into()
}

fn approval_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.step.approval");
  let toggle = Toggle::new(
    "pipeline-hub-step-approval",
    ToggleKind::Switch,
    step.approval_gate,
    label.clone(),
  )
  .hide_label();
  let toggle = if open.locked {
    toggle
  } else {
    let (edit, key) = (open.edit.clone(), step.key.clone());
    toggle.on_change(move |on| {
      edit.change(|current| {
        current.update(&key, |step| step.approval_gate = on);
      });
    })
  };
  rows::row(
    "pipeline-hub-step-approval-row",
    &label,
    None,
    lurq::components::Row::new()
      .flex(1.0)
      .justify(lurq::layout::layout_kind::Justify::End)
      .child(toggle),
  )
}

/// *Repository access*: *Read-only* or *Can write*, which only a profile that can write offers.
fn access_row(
  ctx: &mut Ctx,
  open: &Open<'_>,
  step: &Step,
  (profile, repository): (Option<orchester_core::id::ResourceId>, RepositoryAccess),
) -> lurq::components::Row {
  let label = ctx.t("pipeline_hub.step.access");
  let write = repository == RepositoryAccess::Write;
  let can_write = profile
    .and_then(|id| open.assignables.profile(id))
    .is_some_and(narrowing::can_write);
  let options = vec![
    ("read".to_owned(), ctx.t("pipeline_hub.step.readOnly").to_string()),
    ("write".to_owned(), ctx.t("pipeline_hub.step.canWrite").to_string()),
  ];
  let (edit, key) = (open.edit.clone(), step.key.clone());
  let on_pick: Pick = Arc::new(move |picked| {
    let choice = AccessChoice(if picked == "write" {
      RepositoryAccess::Write
    } else {
      RepositoryAccess::ReadOnly
    });
    edit.change(|current| {
      current.update(&key, |step| choice.apply(step));
    });
  });
  let select = pick::pick(
    ctx,
    pick::Props {
      id: format!("pipeline-hub-step-access-{}-{}", step.key, open.epoch),
      name: label.to_string(),
      value: if write { "write" } else { "read" }.to_owned(),
      options,
      placeholder: None,
      mono: false,
      locked: open.locked || !(can_write || write),
      choosing: false,
      on_pick,
    },
  );
  rows::row("pipeline-hub-step-access-row", &label, None, select)
}

/// *Sources*: what the step reads besides the pipeline's, as pills and *Add*, with how many are its own.
fn sources_row(ctx: &mut Ctx, open: &Open<'_>, step: &Step) -> lurq::components::Row {
  let sources = open.current.sources();
  let counts = ctx.t_args(
    "pipeline_hub.step.sourcesCounts",
    [
      ("own", sources.step(&step.key).len().to_string()),
      ("inherited", sources.pipeline().len().to_string()),
    ],
  );
  let value = sources::view(
    ctx,
    SourcesProps {
      root: open.project.open.repo_root.clone(),
      project: open.project.open.project.name().as_str().into(),
      listing: Listing::Step(step.key.clone()),
      current: open.current.clone(),
      edit: open.edit.clone(),
      locked: open.locked,
    },
  );
  rows::row(
    "pipeline-hub-step-sources-row",
    &ctx.t("pipeline_hub.step.sources"),
    Some(Sub::quiet(counts)),
    value,
  )
}
