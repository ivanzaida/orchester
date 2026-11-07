//! The not-ready card asks again whether the pipeline is ready here when it is shown (the person left the Summary to
//! follow a fix and came back) and when the connections, tools or profiles change (a fill-in, a program path, a
//! secret's value), so it lists what is still missing, and goes once nothing is. A refusal the team server's pinned
//! versions caused ([`NotReadyHere::after_server_started`]) cannot be read here and is shown as it was.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_workspace::{
  messages::Message,
  readiness::{NotReadyHere, NotStarted},
  tasks::outside::Outside,
};

use super::{super::TaskContext, card_of};
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

pub struct NotReadyCard;

impl Component for NotReadyCard {
  type Props = CardProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let CardProps { task, refusal } = ctx.props::<CardProps>().clone();
    let outside = task.project.feed.outside.get();
    let changed = (
      outside.of(Outside::Connections),
      outside.of(Outside::Tools),
      outside.of(Outside::Profiles),
    );
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace, services.vault);
    let (root, id) = (task.project.open.repo_root.clone(), task.detail.task.id());
    let keep = refusal.after_server_started();
    let read = ctx.future((root, id, keep, changed), move |(root, id, keep, _)| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        if keep {
          return Ok(Standing::Kept);
        }
        match workspace.ready_to_start(&root, &vault, id).await {
          Ok(()) => Ok(Standing::Ready),
          Err(NotStarted::NotReadyHere(now)) => Ok(Standing::NotReady(now)),
          Err(NotStarted::Unreadable(message)) => Err::<Standing, Message>(message),
        }
      }
    });
    // While it is read, or when it could not be, the card shows what the start was refused for.
    let shown = match read.state().get().data {
      Some(Standing::Ready) => return lurq::components::Column::new().into(),
      Some(Standing::NotReady(now)) => now,
      Some(Standing::Kept) | None => refusal,
    };
    card_of(ctx, &task, &shown)
  }
}
