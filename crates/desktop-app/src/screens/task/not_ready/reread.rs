//! The not-ready card and the notice over the other tabs ask again whether the pipeline is ready here when it is shown
//! (the person left the Summary to follow a fix and came back) and when the connections, tools or profiles change (a
//! fill-in, a program path, a secret's value), so it lists what is still missing, and goes once nothing is. A refusal
//! the team server's pinned versions caused ([`NotReadyHere::after_server_started`]) cannot be read here and is shown
//! as it was.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  node::Element,
};
use orchester_workspace::{
  messages::Message,
  readiness::{NotReadyHere, NotStarted},
  tasks::outside::Outside,
};

use super::{super::TaskContext, card_of, notice::notice_of};
use crate::services::Services;

#[derive(Clone, PartialEq)]
pub struct CardProps {
  pub task: TaskContext,
  /// What the last start was refused for.
  pub refusal: NotReadyHere,
}

impl DevtoolsInspectable for CardProps {}

/// How the pipeline stands now.
#[derive(Clone, Debug, PartialEq)]
enum Standing {
  /// Not read again: the refusal came from the team server's versions.
  Kept,
  Ready,
  NotReady(NotReadyHere),
}

impl DevtoolsInspectable for Standing {}

/// What the pipeline lacks now, read again as the person's last start read it: `None` once nothing is missing, the
/// refusal as it was while it is read, when it could not be, and when the team server's versions caused it.
fn missing_now(ctx: &mut Ctx, props: &CardProps) -> Option<NotReadyHere> {
  let CardProps { task, refusal } = props.clone();
  let outside = task.project.feed.outside.get();
  let changed = (
    outside.of(Outside::Connections),
    outside.of(Outside::Tools),
    outside.of(Outside::Profiles),
  );
  let services = Services::of(ctx);
  let (workspace, vault) = (services.workspace, services.vault);
  let root = task.project.open.repo_root.clone();
  // The same question the start asked: a team task starts through its server, by the pipeline it names, with the
  // program paths set for the team's servers; a local task by its own pipeline.
  let asked = (
    task.team.is_some(),
    task.detail.task.id(),
    task.detail.task.details().pipeline,
  );
  let keep = refusal.after_server_started();
  let read = ctx.future(
    (root, asked, (keep, refusal.clone()), changed),
    move |(root, (team, id, pipeline), (keep, _), _)| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        if keep {
          return Ok(Standing::Kept);
        }
        let ready = if team {
          workspace.ready_to_start_team(&root, &vault, pipeline).await
        } else {
          workspace.ready_to_start(&root, &vault, id).await
        };
        match ready {
          Ok(()) => Ok(Standing::Ready),
          Err(NotStarted::NotReadyHere(now)) => Ok(Standing::NotReady(now)),
          Err(NotStarted::Unreadable(message)) => Err::<Standing, Message>(message),
        }
      }
    },
  );
  match read.state().get().data {
    Some(Standing::Ready) => None,
    Some(Standing::NotReady(now)) => Some(now),
    Some(Standing::Kept) | None => Some(refusal),
  }
}

/// The card on the Summary.
pub struct NotReadyCard;

impl Component for NotReadyCard {
  type Props = CardProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CardProps>().clone();
    match missing_now(ctx, &props) {
      Some(shown) => card_of(ctx, &props.task, &shown),
      None => Column::new().into(),
    }
  }
}

/// The notice over the other tabs, which reads as the card does, so the two never disagree.
pub struct NotReadyNotice;

impl Component for NotReadyNotice {
  type Props = CardProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CardProps>().clone();
    match missing_now(ctx, &props) {
      Some(shown) => notice_of(ctx, &props.task, &shown),
      None => Column::new().into(),
    }
  }
}
