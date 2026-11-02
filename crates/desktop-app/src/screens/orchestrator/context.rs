//! The context meter in the composer's bar (`06 · Context · near full` `Z2dtIW`, states `Kp4IZ`): how full the session
//! of the conversation shown is. It reads the reading alone, again whenever the project's conversations change (a
//! turn's new reading among them), without the conversation's entries; until its first read it shows the reading the
//! conversation was read with.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_core::{connection::context_use::ContextUse, id::ChatId};
use orchester_workspace::{
  background,
  orchestrator::{chat::context_in, settings::Readiness},
};

use super::{
  View,
  data::{Reads, Shown, revision},
};
use crate::{
  project::ShownProject,
  services::Services,
  ui::context_meter::{Compact, ContextMeterProps, MeterPlace, MeterReading, Over, context_meter},
};

/// The meter of the conversation `reads` shows, while the Orchestrator can answer in it, with what *Compact* does
/// there (`super::compact`); `None` for a new one.
pub(super) fn meter(
  ctx: &mut Ctx,
  view: &View,
  project: &ShownProject,
  (reads, readiness): (&Reads, Option<&Readiness>),
) -> Option<Element> {
  let Shown::Chat(shown) = &reads.shown else {
    return None;
  };
  let model = match readiness? {
    Readiness::Ready { model, .. } => Some(Arc::from(model.as_str())),
    _ => return None,
  };
  let chat = shown.chat.id();
  Some(ctx.mount_keyed::<ChatContext>(
    &format!("orchestrator-context-{chat}"),
    ChatContextProps {
      root: project.open.repo_root.clone(),
      chat,
      model,
      read_with: shown.context,
      compact: super::compact::offer(view, project, shown),
      over: Over(view.mention_anchor.clone()),
    },
  ))
}

#[derive(Clone, Debug, PartialEq)]
struct ChatContextProps {
  root: PathBuf,
  chat: ChatId,
  model: Option<Arc<str>>,
  /// The reading the conversation was read with.
  read_with: Option<ContextUse>,
  compact: Compact,
  /// The composer's box, which the popover opens above.
  over: Over,
}

impl DevtoolsInspectable for ChatContextProps {}

struct ChatContext;

impl Component for ChatContext {
  type Props = ChatContextProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChatContextProps>().clone();
    let services = Services::of(ctx);
    let changes = revision(ctx, &services.runners, &props.root);
    let hold = services.hold;
    let state = ctx
      .future((props.root.clone(), props.chat, changes), move |(root, chat, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || context_in(&root, chat).map(MeterReading))
            .await
            .and_then(|read| read)
        }
      })
      .state()
      .get();
    // A read that failed keeps the reading last shown: the conversation, read from the same database, says why.
    let reading = state.data.map_or(props.read_with, |MeterReading(reading)| reading);
    context_meter(
      ctx,
      ContextMeterProps {
        id: "orchestrator-context".into(),
        reading,
        agent: None,
        model: props.model,
        place: MeterPlace::Bar,
        over: Some(props.over),
        compact: props.compact,
      },
    )
  }
}
