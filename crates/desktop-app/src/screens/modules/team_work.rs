//! A team module's *Work and delivery* (`Work` `Z61je6` in `04 · Module · admin` `MvniR`, `vu2zP` in `04d`, `SJPKO` in
//! `04e`; `docs/design/team-authority.md` §4–5): under the description, its heading in 13/600, then the module's claim,
//! its branch and its delivery to the base, each one strip ([`crate::team::strip`]), 10 apart, with what the last
//! action did not do over them. Taking, releasing and taking over the module's claim, changing its branch (an
//! Admin's, asked in `04b` `InYp8`) and asking for or authorizing its delivery each go through the dispatcher, as the
//! server's capabilities allow them; a branch someone else changed first shows what it is now, to reapply or keep
//! (`05b` `H7MBjm`). The lines' words are in [`super::team_work_lines`].

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::{Column, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_api::v2::{AuthorityErrorCode, AuthorityRef, Subject};
use orchester_core::{
  id::{OperationId, ProjectId},
  run::{BranchName, CommitId},
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  actions::{Action, ClaimAction, ModuleAction},
  background,
  team_work::{DeliveryStatus, TeamDeliveryState},
};

use super::team_work_lines as lines;
use crate::{
  services::Services,
  team::{
    TeamSource,
    act::{Acted, Acting},
    deliver::{self, Asked},
    pages::TeamModule,
    view::Standing,
  },
  theme::{ExtraSpacing, TaskText},
};

#[derive(Clone, PartialEq)]
pub struct WorkProps {
  pub project: ProjectId,
  pub module: Arc<TeamModule>,
  pub source: TeamSource,
  pub standing: Standing,
}

impl lurq::app::component::DevtoolsInspectable for WorkProps {}

/// What a person asks of a team module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleAsk {
  Claim,
  TakeOver,
  Release,
  /// Give it this branch (an Admin's).
  Branch(String),
  /// Ask for its delivery, on this authorization or on Automatic merge.
  Deliver(Option<OperationId>),
  /// Authorize the delivery that waits for it, then ask for it on that authorization.
  Authorize,
}

/// What came of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleAnswer {
  Acted(ModuleAsk, Acted),
  Asked(ModuleAsk, Asked),
}

impl lurq::app::component::DevtoolsInspectable for ModuleAnswer {}

/// A delivery of the module as this computer recorded it, as the page shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShownDelivery {
  pub status: DeliveryStatus,
  pub branch: BranchName,
  pub base_branch: BranchName,
  pub source_commit: CommitId,
  pub base_commit: CommitId,
  /// The merge here failed.
  pub merge_failed: bool,
}

impl lurq::app::component::DevtoolsInspectable for ShownDelivery {}

impl From<&TeamDeliveryState> for ShownDelivery {
  fn from(state: &TeamDeliveryState) -> Self {
    Self {
      status: state.status.clone(),
      branch: state.branch.clone(),
      base_branch: state.base_branch.clone(),
      source_commit: state.source_commit.clone(),
      base_commit: state.base_commit.clone(),
      merge_failed: matches!(state.merge, Some(orchester_store::DeliveryMerge::Failed { .. })),
    }
  }
}

/// The section's state across renders.
pub struct ModuleWork {
  /// Change branch's question is open, and the name typed in it.
  changing: Signal<bool>,
  branch: Signal<String>,
  /// The person kept the branch someone else gave the module first; reset by the next ask.
  kept: Signal<bool>,
  /// Counts the asks that ended, so the deliveries recorded here are read again after each.
  answered: Signal<u64>,
}

pub type WorkControl = FutureAction<ModuleAsk, ModuleAnswer, ()>;

impl Component for ModuleWork {
  type Props = WorkProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      changing: ctx.signal(false),
      branch: ctx.signal(String::new()),
      kept: ctx.signal(false),
      answered: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WorkProps>().clone();
    let delivery = self.delivery(ctx, &props);
    let action = self.action(ctx, &props, delivery.clone());
    let view = lines::View {
      props: &props,
      action: &action,
      delivery: delivery.as_ref(),
      changing: &self.changing,
      branch: &self.branch,
      kept: &self.kept,
    };
    let heading = Text::new(&ctx.t("modules.team.work.heading"))
      .id("module-team-work-heading")
      .variant(TaskText::SectionTitle.role())
      .color(PaletteColor::TextPrimary);
    Column::new()
      .id("module-team-work")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(heading)
      .with_children(lines::refusal(ctx, &view))
      .child(lines::claim(ctx, &view))
      .with_children(lines::conflict(ctx, &view))
      .child(lines::branch(ctx, &view))
      .child(lines::delivery(ctx, &view))
      .with_children(lines::branch_dialog(ctx, &view))
  }
}

impl ModuleWork {
  /// The newest delivery of the module recorded here, read again after every ask.
  fn delivery(&self, ctx: &mut Ctx, props: &WorkProps) -> Option<ShownDelivery> {
    let services = Services::of(ctx);
    let root = props.source.root().to_owned();
    let module = props.module.module.id();
    let deps = (root, module, self.answered.get());
    ctx
      .future(deps, move |(root, module, _)| {
        let (servers, programs) = (services.servers.clone(), services.programs.clone());
        async move {
          let read = background::run(move || deliver::deliveries(root, (servers, programs), module)).await;
          read.map(|all| all.last().map(ShownDelivery::from)).map_err(|_| ())
        }
      })
      .state()
      .get()
      .data
      .flatten()
  }

  /// The section's action on the module as the page shows it; made on every render. A branch change closes the
  /// question once the server answered it, but for a branch it refused as taken, which the question says under its
  /// field: the page says the rest.
  fn action(&self, ctx: &mut Ctx, props: &WorkProps, delivery: Option<ShownDelivery>) -> WorkControl {
    let services = Services::of(ctx);
    let (acting, programs) = (Acting::of(&services), services.programs.clone());
    let (props, answered) = (props.clone(), self.answered.clone());
    let (changing, kept) = (self.changing.clone(), self.kept.clone());
    ctx.future_action(move |ask: ModuleAsk| {
      let (acting, programs, props, delivery) = (acting.clone(), programs.clone(), props.clone(), delivery.clone());
      let (answered, changing) = (answered.clone(), changing.clone());
      kept.set(false);
      async move {
        let answer = carry_out(ask, (&acting, programs), &props, delivery).await;
        let taken = matches!(
          answer,
          ModuleAnswer::Acted(_, Acted::Refused(AuthorityErrorCode::Invalid))
        );
        if matches!(answer, ModuleAnswer::Acted(ModuleAsk::Branch(_), _)) && !taken {
          changing.set(false);
        }
        answered.update(|count| *count += 1);
        Ok(answer)
      }
    })
  }
}

async fn carry_out(
  ask: ModuleAsk,
  (acting, programs): (&Acting, Programs),
  props: &WorkProps,
  delivery: Option<ShownDelivery>,
) -> ModuleAnswer {
  let module = &props.module;
  let (source, id) = (&props.source, module.module.id());
  let subject = Subject::Module { id };
  let act = |action| async move { acting.act(source, action).await.0 };
  let action = match (ask.clone(), module.claim.as_ref().map(|claim| claim.id)) {
    (ModuleAsk::Claim, _) => Action::Claim(ClaimAction::Acquire { subject }),
    (ModuleAsk::TakeOver, Some(observed)) => Action::Claim(ClaimAction::TakeOver { subject, observed }),
    (ModuleAsk::Release, Some(claim_id)) => Action::Claim(ClaimAction::Release {
      authority: AuthorityRef {
        subject,
        claim_id,
        work_id: None,
      },
    }),
    (ModuleAsk::Branch(named), _) => {
      let Ok(branch) = BranchName::new(named.trim()) else {
        return ModuleAnswer::Acted(ask, Acted::Refused(AuthorityErrorCode::Invalid));
      };
      Action::Module(ModuleAction::AssignBranch {
        module: id,
        read: module.revision,
        branch,
      })
    }
    (ModuleAsk::Deliver(authorization), Some(claim)) => {
      let target = (id, claim, module.branch.clone());
      return ModuleAnswer::Asked(
        ask,
        deliver::request(acting, source, programs, target, authorization).await,
      );
    }
    (ModuleAsk::Authorize, Some(claim)) => return authorize(ask, (acting, programs), props, (delivery, claim)).await,
    (ModuleAsk::TakeOver | ModuleAsk::Release | ModuleAsk::Deliver(_) | ModuleAsk::Authorize, None) => {
      return ModuleAnswer::Acted(ask, Acted::Failed);
    }
  };
  ModuleAnswer::Acted(ask, act(action).await)
}

/// An Admin who holds the module here authorizes the delivery that waits for it, then asks for it on that.
async fn authorize(
  ask: ModuleAsk,
  (acting, programs): (&Acting, Programs),
  props: &WorkProps,
  (delivery, claim): (Option<ShownDelivery>, orchester_core::id::ClaimId),
) -> ModuleAnswer {
  let Some(delivery) = delivery else {
    return ModuleAnswer::Acted(ask, Acted::Failed);
  };
  let id = props.module.module.id();
  let commits = (delivery.source_commit.clone(), delivery.base_commit.clone());
  match deliver::authorize(acting, &props.source, (id, claim, delivery.branch.clone()), commits).await {
    Ok(authorization) => {
      let target = (id, claim, props.module.branch.clone());
      ModuleAnswer::Asked(
        ask,
        deliver::request(acting, &props.source, programs, target, Some(authorization)).await,
      )
    }
    Err(acted) => ModuleAnswer::Acted(ask, acted),
  }
}
