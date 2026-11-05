//! A task's Services tab (ORC-265, `06 · Task · Services` `cjvRr` in `Agent testing · redesign` `eWydp`;
//! `docs/architecture/agent-testing.md`, "Screens"): the services the steps of the task's run at work started, each
//! with its state, port, address and uptime, Restart and Stop through the runner, and the selected one's live log.
//!
//! The run's runner keeps its services; while the run runs on a runner that is online, the frame asks it for them
//! every second ([`reader`]) and shares what it answered with the page's tabs ([`ServicesReads`]): the Services tab
//! counts them, with a dot in the tone of the worst of them (`Dot` `JrkWf`: `danger` once one crashed, `info` while
//! one starts, `success` while all are ready). The tab lists them ([`tab`]); each row's actions go to the runner
//! ([`actions`]); the selected service's log follows live ([`log`]), or opens whole ([`full_log`]).

mod actions;
mod full_log;
mod log;
mod reader;
mod row;
mod since;
mod tab;
mod table;
mod words;

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};
use orchester_core::id::RunId;
use orchester_ipc::services::{RunService, RunServiceState};
use orchester_workspace::messages::Message;

use super::TaskContext;

/// The run's services as its runner last listed them.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Listed {
  /// Not asked yet.
  #[default]
  Reading,
  /// The services the steps of `run` at work started, in the order they started.
  Shown { run: RunId, services: Arc<[RunService]> },
  /// The runner did not answer, and why.
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for Listed {}

/// What the frame read of the run's services, which the page's tabs share.
#[derive(Clone)]
pub struct ServicesReads {
  pub listed: Signal<Listed>,
}

impl ServicesReads {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      listed: ctx.signal(Listed::default()),
    }
  }

  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the task frame provides its services' reads before mounting its parts")
  }

  /// The services of `task`'s run while it works on a runner that is online; `None` while they are not known yet.
  fn shown(&self, task: &TaskContext) -> Option<Option<Arc<[RunService]>>> {
    let Some(run) = live_run(task) else {
      return Some(None);
    };
    match self.listed.get() {
      Listed::Shown { run: listed, services } if listed == run => Some(Some(services)),
      Listed::Reading | Listed::Shown { .. } | Listed::Failed(_) => None,
    }
  }
}

/// The run whose services live now: the task's latest run while it runs on a runner that is online.
fn live_run(task: &TaskContext) -> Option<RunId> {
  let run = task.detail.run.as_ref()?;
  (task.online && super::read::runs(&task.detail)).then(|| run.id())
}

/// The frame's reader of the run's services while they live (`reader`).
pub fn reader(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let run = live_run(task)?;
  Some(reader::mount(ctx, task.project.open.repo_root.clone(), run))
}

/// The Services tab's count and dot: how many services the run's steps at work started, and the tone of the worst of
/// them; `None` while none run.
pub fn count(ctx: &mut Ctx, task: &TaskContext) -> Option<(usize, Option<PaletteColor>)> {
  let services = ServicesReads::of(ctx).shown(task).flatten()?;
  if services.is_empty() {
    return None;
  }
  let states = || services.iter().map(|service| service.state);
  let dot = if states().any(|state| matches!(state, RunServiceState::Crashed { .. })) {
    Some(PaletteColor::Danger)
  } else if states().any(|state| state == RunServiceState::Starting) {
    Some(PaletteColor::Info)
  } else if states().all(|state| state == RunServiceState::Ready) {
    Some(PaletteColor::Success)
  } else {
    None
  };
  Some((services.len(), dot))
}

/// The Services tab's mount point.
pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  tab::mount(ctx, task)
}
