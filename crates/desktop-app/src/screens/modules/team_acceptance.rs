//! *Delivery* on a team module's Summary (`docs/plan/orchestrator-autonomy/OA-32-…`): the group of New module and
//! Edit module (`super::form_delivery`, `M01 · Edit module · its own branch`) without its *Its own branch* switch, as
//! the team assigns a team module's branch: *Acceptance pipeline*, none or one of the team's pipelines, and *Deliver
//! when accepted*, off and faded while no acceptance pipeline is chosen. Both are this computer's choice, which the
//! team server does not keep, so a member chooses them as an Admin does; each change is kept at once, and a change that
//! could not be kept says why under the group.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ModuleId, ResourceId},
  module::delivery::AcceptanceChoice,
};
use orchester_workspace::{
  background,
  messages::Message,
  modules::team_delivery::{choice_in, choose_in},
};

use super::form_delivery::{AcceptancePick, OwnPick, acceptance_row, deliver_row};
use crate::{
  project::{ShownProject, TasksState},
  ui::{field::field_error, message::message_text, setting_rows::SettingGroup},
};

#[derive(Clone, PartialEq)]
pub struct AcceptanceProps {
  pub project: ShownProject,
  pub module: ModuleId,
}

impl DevtoolsInspectable for AcceptanceProps {}

/// The choice as last read or kept; `None` until it is read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Kept(Option<AcceptanceChoice>);

impl DevtoolsInspectable for Kept {}

/// What a change keeps, and where.
#[derive(Clone)]
struct Choose {
  root: PathBuf,
  module: ModuleId,
  choice: AcceptanceChoice,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Refused(Message);

impl DevtoolsInspectable for Refused {}

pub struct TeamAcceptance {
  own: Signal<OwnPick>,
  acceptance: Signal<AcceptancePick>,
  kept: Signal<Kept>,
  save: FutureAction<Choose, Kept, Refused>,
}

impl Component for TeamAcceptance {
  type Props = AcceptanceProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<AcceptanceProps>().clone();
    let save = ctx.future_action(|choose: Choose| async move {
      background::run(move || choose_in(&choose.root, choose.module, choose.choice))
        .await
        .map_err(Refused)?
        .map(|choice| Kept(Some(choice)))
        .map_err(Refused)
    });
    let own = ctx.signal(OwnPick {
      own_branch: true,
      deliver_when_accepted: false,
    });
    let acceptance = ctx.signal(AcceptancePick::default());
    let kept = ctx.signal(Kept::default());
    let keep = Keep {
      root: props.project.open.repo_root.clone(),
      module: props.module,
      own: own.clone(),
      acceptance: acceptance.clone(),
      kept: kept.clone(),
      save: save.clone(),
    };
    let on_own = keep.clone();
    ctx.watch(&own, move |_| on_own.changed());
    ctx.watch(&acceptance, move |_| keep.changed());
    Self {
      own,
      acceptance,
      kept,
      save,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AcceptanceProps>().clone();
    self.seed(ctx, &props);
    let pipelines: Vec<(ResourceId, String)> = match props.project.tasks() {
      TasksState::Ready(tasks) => tasks
        .pipelines
        .iter()
        .map(|pipeline| (pipeline.id, pipeline.name.to_string()))
        .collect(),
      _ => Vec::new(),
    };
    let rows = vec![
      acceptance_row(ctx, &pipelines, &self.acceptance),
      deliver_row(
        ctx,
        (&self.own, &ctx.t("modules.team.acceptance.deliverHint")),
        self.acceptance.get().0.is_some(),
      ),
    ];
    let group = SettingGroup::new("module-delivery", ctx.t("modules.form.delivery.title"))
      .rows(rows)
      .foot(&ctx.t("modules.team.acceptance.foot"));
    let refused =
      self.save.state().get().error.map(|Refused(message)| -> Element {
        field_error("module-delivery-error", &message_text(ctx, &message)).into()
      });
    Column::new()
      .id("module-team-delivery")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(group)
      .with_children(refused)
  }
}

impl TeamAcceptance {
  /// Fills the fields once with the choice kept on this computer, read off the UI thread.
  fn seed(&self, ctx: &mut Ctx, props: &AcceptanceProps) {
    let key = (props.project.open.repo_root.clone(), props.module);
    let read = ctx
      .future(key, |(root, module): (PathBuf, ModuleId)| async move {
        background::run(move || choice_in(&root, module))
          .await?
          .map(|choice| Kept(Some(choice)))
      })
      .state()
      .get();
    let Some(Kept(Some(choice))) = read.data.filter(|_| self.kept.get_untracked().0.is_none()) else {
      return;
    };
    // The fields first and what was kept last: until it is set, their changes are the stored choice, not the person's,
    // and are not kept again.
    self.acceptance.set(AcceptancePick(choice.acceptance));
    self
      .own
      .update(|pick| pick.deliver_when_accepted = choice.deliver_when_accepted);
    self.kept.set(Kept(Some(choice)));
  }
}

/// Keeps what the fields hold whenever it differs from what was last kept, once the stored choice was read.
#[derive(Clone)]
struct Keep {
  root: PathBuf,
  module: ModuleId,
  own: Signal<OwnPick>,
  acceptance: Signal<AcceptancePick>,
  kept: Signal<Kept>,
  save: FutureAction<Choose, Kept, Refused>,
}

impl Keep {
  fn changed(&self) {
    let Kept(Some(kept)) = self.kept.get_untracked() else {
      return;
    };
    let acceptance = self.acceptance.get_untracked().0;
    let choice = AcceptanceChoice {
      acceptance,
      deliver_when_accepted: acceptance.is_some() && self.own.get_untracked().deliver_when_accepted,
    };
    if choice == kept {
      return;
    }
    self.kept.set(Kept(Some(choice)));
    self.save.run(Choose {
      root: self.root.clone(),
      module: self.module,
      choice,
    });
  }
}
