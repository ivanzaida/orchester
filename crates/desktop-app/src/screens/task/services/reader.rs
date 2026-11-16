//! Asks the run's runner for its services every second while they live, and keeps what it answered in the frame's
//! [`ServicesReads`] when it changed. Draws nothing.

use std::{path::PathBuf, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
  },
  node::Element,
};
use orchester_core::id::RunId;
use orchester_ipc::services::{ServiceCommand, ServiceReply};
use orchester_workspace::{
  runner::{RequestError, RunRequestError},
  tasks::run::control_message,
};

use super::{Listed, ServicesReads};
use crate::services::Services;

/// How often the runner is asked.
const POLL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReaderProps {
  root: PathBuf,
  run: RunId,
}

impl DevtoolsInspectable for ReaderProps {}

/// The reader of `run`'s services in the project at `root`.
pub(super) fn mount(ctx: &mut Ctx, root: PathBuf, run: RunId) -> Element {
  ctx.mount_keyed::<ServicesReader>(&format!("task-services-reader-{run}"), ReaderProps { root, run })
}

struct ServicesReader;

impl Component for ServicesReader {
  type Props = ReaderProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ReaderProps { root, run } = ctx.props::<ReaderProps>().clone();
    let runners = Services::of(ctx).runners;
    let listed = ctx
      .stream((root, run), move |(root, run), emitter: StreamEmitter<Listed, ()>| {
        let runners = runners.clone();
        async move {
          loop {
            let listed = match runners.services(&root, ServiceCommand::List { run }).await {
              Ok(ServiceReply::Listed { services, .. }) => Listed::Shown {
                run,
                services: services.into(),
              },
              Ok(other) => {
                tracing::error!(?other, "unexpected answer to the run's services");
                Listed::Failed(control_message(&RunRequestError::Request(RequestError::Lost)))
              }
              Err(error) => {
                tracing::debug!(%error, "the run's services cannot be listed now");
                Listed::Failed(control_message(&error))
              }
            };
            if !emitter.emit(listed) {
              return;
            }
            tokio::time::sleep(POLL).await;
          }
        }
      })
      .state()
      .get()
      .data;
    let reads = ServicesReads::of(ctx);
    if let Some(listed) = listed
      && reads.listed.get_untracked() != listed
    {
      reads.listed.set(listed);
    }
    Element::new()
  }
}
