//! A team module's *Work and delivery* (`Work` `Z61je6` in `04 · Module · admin` `MvniR`, `vu2zP` in `04d`, `SJPKO` in
//! `04e`; `docs/design/team-authority.md` §4–5): under the description, its heading in 13/600, then the module's claim,
//! its branch and its delivery to the base, each one strip ([`crate::team::strip`]), 10 apart, with what the last
//! action did not do over them. Taking, releasing and taking over the module's claim, changing its branch (an
//! Admin's, asked in `04b` `InYp8`), asking for its delivery and authorizing the one that waits for an Admin, as the
//! module's read names it, each go through the dispatcher, as the server's capabilities allow them; a branch someone
//! else changed first shows what it is now, to reapply or keep (`05b` `H7MBjm`). The lines' words are in
//! [`super::team_work_lines`] and [`super::team_work_delivery`].

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
  id::{ClaimId, OperationId, ProjectId},
  run::{BranchName, CommitId},
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  actions::{Action, ClaimAction, ModuleAction},
  background,
  team_work::{DeliveryStatus, TeamDeliveryState},
};

use super::{team_work_delivery as delivery, team_work_lines as lines};
use crate::{
  screens::settings::orchester::PersonOnly,
  services::Services,
  team::{
    TeamSource,
    act::{Acted, Acting},
    deliver::{self, Asked},
    pages::{TeamDelivery, TeamModule},
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
  /// Authorize the delivery that waits for an Admin, of exactly the commits the module's read names; then, when this
  /// computer holds the module, ask for it on that authorization.
  Authorize,
  /// Ask again for the delivery an Admin authorized while it waited, of exactly its commits: this request takes the
  /// merge.
  Merge,
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

impl ShownDelivery {
  /// Whether it is of exactly the commits of the server's open delivery `pending`.
  pub fn binds(&self, pending: &TeamDelivery) -> bool {
    self.source_commit == pending.source_commit && self.base_commit == pending.base_commit
  }

  /// Whether the server let it merge here: its merge or report is under way here, or ended.
  pub const fn merged_here(&self) -> bool {
    matches!(
      self.status,
      DeliveryStatus::Authorized { .. }
        | DeliveryStatus::AuthorizedAutomatically
        | DeliveryStatus::Delivered
        | DeliveryStatus::Failed
    )
  }
}

/// What this computer knows of the module's deliveries: the newest it recorded, and the branch its checkout has checked
/// out now, which a delivery asked for here lands on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorded {
  pub newest: Option<ShownDelivery>,
  pub checked_out: Option<BranchName>,
}

impl lurq::app::component::DevtoolsInspectable for Recorded {}

impl Recorded {
  /// The branch the server's open delivery `pending` lands on here: the one the delivery recorded here of its commits
  /// names, else the one the checkout has checked out.
  pub fn base_of(&self, pending: &TeamDelivery) -> Option<BranchName> {
    let recorded = self.newest.as_ref().filter(|newest| newest.binds(pending));
    recorded
      .map(|newest| newest.base_branch.clone())
      .or_else(|| self.checked_out.clone())
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
  /// Take over and Authorize, which only a person presses.
  person: PersonOnly,
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
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<WorkProps>().clone();
    let recorded = self.recorded(ctx, &props);
    let action = self.action(ctx, &props, recorded.clone());
    let view = lines::View {
      props: &props,
      action: &action,
      recorded: &recorded,
      changing: &self.changing,
      branch: &self.branch,
      kept: &self.kept,
      person: &self.person,
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
      .with_children(lines::person_only(ctx, &view, &[lines::AGAIN]))
      .child(lines::claim(ctx, &view))
      .with_children(lines::person_only(ctx, &view, &[lines::TAKE_OVER]))
      .with_children(lines::conflict(ctx, &view))
      .child(lines::branch(ctx, &view))
      .child(delivery::delivery(ctx, &view))
      .with_children(lines::person_only(ctx, &view, &[delivery::AUTHORIZE]))
      .with_children(lines::branch_dialog(ctx, &view))
  }
}

impl ModuleWork {
  /// The newest delivery of the module recorded here and the branch the checkout has checked out, read again after
  /// every ask.
  fn recorded(&self, ctx: &mut Ctx, props: &WorkProps) -> Recorded {
    let services = Services::of(ctx);
    let root = props.source.root().to_owned();
    let module = props.module.module.id();
    let deps = (root, module, self.answered.get());
    ctx
      .future(deps, move |(root, module, _)| {
        let (servers, programs) = (services.servers.clone(), services.programs.clone());
        async move {
          let read = background::run(move || {
            let all = deliver::deliveries(root.clone(), (servers, programs.clone()), module);
            Recorded {
              newest: all.last().map(ShownDelivery::from),
              checked_out: deliver::checked_out(&programs, &root),
            }
          });
          read.await.map_err(|_| ())
        }
      })
      .state()
      .get()
      .data
      .unwrap_or_default()
  }

  /// The section's action on the module as the page shows it; made on every render. A branch change closes the
  /// question once the server answered it, but for a branch it refused as taken, which the question says under its
  /// field: the page says the rest.
  fn action(&self, ctx: &mut Ctx, props: &WorkProps, recorded: Recorded) -> WorkControl {
    let services = Services::of(ctx);
    let (acting, programs) = (Acting::of(&services), services.programs.clone());
    let (props, answered) = (props.clone(), self.answered.clone());
    let (changing, kept) = (self.changing.clone(), self.kept.clone());
    ctx.future_action(move |ask: ModuleAsk| {
      let (acting, programs, props, recorded) = (acting.clone(), programs.clone(), props.clone(), recorded.clone());
      let (answered, changing) = (answered.clone(), changing.clone());
      kept.set(false);
      async move {
        let answer = carry_out(ask, (&acting, programs), &props, &recorded).await;
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
  recorded: &Recorded,
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
    (ModuleAsk::Authorize, Some(claim)) => return authorize(ask, (acting, programs), props, (recorded, claim)).await,
    (ModuleAsk::Merge, Some(claim)) => return merge(ask, (acting, programs), props, (recorded, claim), None).await,
    (
      ModuleAsk::TakeOver | ModuleAsk::Release | ModuleAsk::Deliver(_) | ModuleAsk::Authorize | ModuleAsk::Merge,
      None,
    ) => {
      return ModuleAnswer::Acted(ask, Acted::Failed);
    }
  };
  ModuleAnswer::Acted(ask, act(action).await)
}

/// An Admin authorizes the delivery that waits for one, of exactly the commits the module's read names, under the
/// module's claim and on its branch; one who holds the module here then asks for that delivery on the authorization.
async fn authorize(
  ask: ModuleAsk,
  (acting, programs): (&Acting, Programs),
  props: &WorkProps,
  (recorded, claim): (&Recorded, ClaimId),
) -> ModuleAnswer {
  let module = &props.module;
  let (Some(pending), Ok(branch)) = (module.pending.as_ref(), BranchName::new(&module.branch)) else {
    return ModuleAnswer::Acted(ask, Acted::Failed);
  };
  let commits = (pending.source_commit.clone(), pending.base_commit.clone());
  let id = module.module.id();
  match deliver::authorize(acting, &props.source, (id, claim, branch), commits).await {
    Ok(authorization) if module.claim.as_ref().is_some_and(|claim| claim.here) => {
      merge(ask, (acting, programs), props, (recorded, claim), Some(authorization)).await
    }
    Ok(_) => ModuleAnswer::Acted(ask, Acted::Done),
    Err(acted) => ModuleAnswer::Acted(ask, acted),
  }
}

/// The holder here asks for exactly the delivery the module's read names, citing `authorization` when it has one:
/// once an Admin authorized it, this request takes the merge. It lands on the branch the delivery recorded here names,
/// or the one the checkout has checked out.
async fn merge(
  ask: ModuleAsk,
  (acting, programs): (&Acting, Programs),
  props: &WorkProps,
  (recorded, claim): (&Recorded, ClaimId),
  authorization: Option<OperationId>,
) -> ModuleAnswer {
  let module = &props.module;
  let Some(pending) = module.pending.as_ref() else {
    return ModuleAnswer::Acted(ask, Acted::Failed);
  };
  let Ok(branch) = BranchName::new(&module.branch) else {
    return ModuleAnswer::Asked(ask, Asked::BranchNotFound(module.branch.clone()));
  };
  let Some(base) = recorded.base_of(pending) else {
    return ModuleAnswer::Asked(ask, Asked::NoBase);
  };
  let tips = (branch, pending.source_commit.clone(), base, pending.base_commit.clone());
  let target = (module.module.id(), claim, tips);
  let asked = deliver::request_tips(acting, &props.source, programs, target, authorization).await;
  ModuleAnswer::Asked(ask, asked)
}
