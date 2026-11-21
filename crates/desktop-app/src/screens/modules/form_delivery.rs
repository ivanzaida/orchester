//! *Delivery* in New module and Edit module, under the property chips (`M01 · Edit module · its own branch` and
//! `Parts · Module form` on the board `Orchestrator autonomy · frames to approve`;
//! `docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`): a group of settings' rows (`ui::setting_rows`). *Its own
//! branch*, a switch, off for a new module; and only while it is on, *Acceptance pipeline*, one of the project's
//! published pipelines or none, and *Deliver when accepted*, a switch that stays off, with why, while no acceptance
//! pipeline is chosen. The choice is saved with the form (`super::save`), on this computer; the branch itself is made
//! when the module's first task starts.

use std::path::PathBuf;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureState},
  },
  components::SelectOption,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ResourceId, module::Module};
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  messages::{self, Message},
  modules::delivery::OwnChange,
  tasks::data::published_pipelines,
};

use super::form::FormProps;
use crate::ui::{
  field::SelectField,
  icon::Icon,
  setting_rows::{SettingGroup, SettingRow},
  toggle::{Toggle, ToggleKind},
};

/// The acceptance pipeline's box (`Select · Module acceptance` in M01).
const PIPELINE_WIDTH: f32 = 200.0;

/// What the form holds of the module's own branch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnPick {
  pub own_branch: bool,
  pub deliver_when_accepted: bool,
}

impl DevtoolsInspectable for OwnPick {}

/// The acceptance pipeline the form holds, or none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AcceptancePick(pub Option<ResourceId>);

impl DevtoolsInspectable for AcceptancePick {}

/// The project's published pipelines by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pipelines(Vec<(ResourceId, String)>);

impl DevtoolsInspectable for Pipelines {}

/// `module`'s own branch as the form's fields start with it; a new module has none.
pub fn read(module: Option<&Module>) -> (OwnPick, AcceptancePick) {
  match module.and_then(Module::own_branch) {
    Some(own) => (
      OwnPick {
        own_branch: true,
        deliver_when_accepted: own.deliver_when_accepted,
      },
      AcceptancePick(own.acceptance),
    ),
    None => (OwnPick::default(), AcceptancePick::default()),
  }
}

/// What a save changes of `module`'s own branch for the fields `(own, acceptance)`; `None` when they hold what the
/// module has. *Deliver when accepted* goes off with the acceptance pipeline, which it cannot be without.
pub fn change(module: Option<&Module>, (own, acceptance): (OwnPick, AcceptancePick)) -> Option<OwnChange> {
  let (was, was_acceptance) = read(module);
  if !own.own_branch {
    return was.own_branch.then(|| OwnChange {
      own_branch: Some(false),
      ..OwnChange::default()
    });
  }
  let delivers = own.deliver_when_accepted && acceptance.0.is_some();
  let same = was.own_branch && was.deliver_when_accepted == delivers && was_acceptance == acceptance;
  (!same).then_some(OwnChange {
    own_branch: Some(true),
    acceptance: Some(acceptance.0),
    deliver_when_accepted: Some(delivers),
  })
}

/// The group, with the rows its switch shows.
pub fn group(
  ctx: &mut Ctx,
  props: &FormProps,
  (own, acceptance): (&Signal<OwnPick>, &Signal<AcceptancePick>),
) -> Element {
  let pipelines = pipelines(ctx, props);
  let held = own.get();
  let branch = props
    .module
    .as_ref()
    .and_then(|module| module.own_branch())
    .map(|own| own.branch.to_string());
  let hint = match (&branch, held.own_branch) {
    (Some(branch), true) => ctx.t_args(
      "modules.form.delivery.ownBranch.hintNamed",
      [("branch", branch.as_str())],
    ),
    (_, true) => ctx.t("modules.form.delivery.ownBranch.hintOn"),
    (_, false) => ctx.t("modules.form.delivery.ownBranch.hintOff"),
  };
  let label = ctx.t("modules.form.delivery.ownBranch.label");
  let switched = own.clone();
  let own_row = SettingRow::new("module-own-branch-row", label.clone())
    .hint(hint)
    .control(
      Toggle::new("module-own-branch", ToggleKind::Switch, held.own_branch, label)
        .hide_label()
        .on_change(move |on| switched.update(|pick| pick.own_branch = on)),
    )
    .render(ctx);
  let mut rows = vec![own_row];
  if held.own_branch {
    let listed = pipelines
      .data
      .as_ref()
      .map_or(&[][..], |pipelines| pipelines.0.as_slice());
    rows.push(acceptance_row(ctx, listed, acceptance));
    let hint = ctx.t("modules.form.delivery.deliver.hint");
    rows.push(deliver_row(ctx, (own, &hint), acceptance.get().0.is_some()));
  }
  SettingGroup::new("module-delivery", ctx.t("modules.form.delivery.title"))
    .rows(rows)
    .foot(&ctx.t("modules.form.delivery.foot"))
    .into()
}

/// *Acceptance pipeline*: none, or one of `pipelines`, the project's published pipelines by name (a team project's are
/// the team's).
pub(super) fn acceptance_row(
  ctx: &mut Ctx,
  pipelines: &[(ResourceId, String)],
  acceptance: &Signal<AcceptancePick>,
) -> Element {
  let label = ctx.t("modules.form.delivery.acceptance.label");
  let mut options = vec![SelectOption::new(
    AcceptancePick(None),
    ctx.t("modules.form.delivery.acceptance.none").to_string(),
  )];
  options.extend(
    pipelines
      .iter()
      .map(|(id, name)| SelectOption::new(AcceptancePick(Some(*id)), name.clone())),
  );
  // An id of its own: the row's label is `module-acceptance-label`, which the select would also name its value.
  let select = SelectField::new("module-acceptance-pipeline", label.clone(), acceptance.clone(), options)
    .leading(Icon::Workflow)
    .width(Dimension::Px(PIPELINE_WIDTH))
    .render(ctx);
  SettingRow::new("module-acceptance", label)
    .hint(ctx.t("modules.form.delivery.acceptance.hint"))
    .control(select)
    .render(ctx)
}

/// *Deliver when accepted*, with `hint`: off and faded, with why, while the module names no acceptance pipeline.
pub(super) fn deliver_row(ctx: &mut Ctx, (own, hint): (&Signal<OwnPick>, &str), accepted: bool) -> Element {
  let label = ctx.t("modules.form.delivery.deliver.label");
  let on = accepted && own.get().deliver_when_accepted;
  let switch = Toggle::new("module-deliver-when-accepted", ToggleKind::Switch, on, label.clone()).hide_label();
  let row = SettingRow::new("module-deliver-when-accepted-row", label);
  if accepted {
    let switched = own.clone();
    row
      .hint(hint.to_owned())
      .control(switch.on_change(move |on| switched.update(|pick| pick.deliver_when_accepted = on)))
      .render(ctx)
  } else {
    row
      .hint(ctx.t("modules.form.delivery.deliver.needsAcceptance"))
      .control(switch)
      .faded()
      .render(ctx)
  }
}

/// The project's published pipelines, read once for the form.
fn pipelines(ctx: &mut Ctx, props: &FormProps) -> FutureState<Pipelines, Message> {
  let root = props.project.open.repo_root.clone();
  ctx
    .future(root, move |root: PathBuf| async move {
      background::run(move || {
        let db = ProjectDb::open(&root).map_err(|error| messages::store_error(&error))?;
        published_pipelines(&db)
          .map(|pipelines| {
            Pipelines(
              pipelines
                .into_iter()
                .map(|pipeline| (pipeline.id, pipeline.name.to_string()))
                .collect(),
            )
          })
          .map_err(|error| messages::store_error(&error))
      })
      .await?
    })
    .state()
    .get()
}
