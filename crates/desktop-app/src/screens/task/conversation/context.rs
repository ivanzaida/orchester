//! The context meter of the step at work (`05 · Conversation · context near full` `bhtch`, states `bxHFm`): on its
//! divider, after its agent and model, and in the composer's bar after the free space. Earlier steps' dividers carry
//! none. It reads the step's reading alone, again as the run's changes arrive (its agent's turns among them), without
//! the run's detail or its transcript; until its first read it shows the reading the task was read with.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::ElementRef,
  node::Element,
};
use orchester_core::{
  connection::context_use::ContextUse,
  id::{RunId, StepExecutionId},
};
use orchester_workspace::{
  background,
  tasks::{
    context_uses::of_execution_in,
    detail::{RunView, StepView},
  },
};

use crate::{
  project::{ShownProject, run_changes, settled_read::settled_read},
  services::Services,
  ui::context_meter::{Compact, ContextMeterProps, MeterPlace, MeterReading, Over, context_meter},
};

/// The meter on the divider of `execution` of `run`, while it is its step's latest and that step's session is at work
/// ([`RunView::session_at_work`]).
pub(super) fn on_divider(
  ctx: &mut Ctx,
  project: &ShownProject,
  run: &RunView,
  execution: Option<StepExecutionId>,
) -> Option<Element> {
  let step = run.session_at_work(execution?)?;
  let id = format!("conversation-context-step-{:02}", step.number);
  meter(ctx, project, (run, step), (id, MeterPlace::Divider, None))
}

/// The meter in the composer's bar, of the first step of `run` whose session is at work; its popover opens above the
/// composer's box `over`.
pub(super) fn in_composer(ctx: &mut Ctx, project: &ShownProject, run: &RunView, over: &ElementRef) -> Option<Element> {
  let step = run.sessions_at_work().next()?;
  let place = (
    "conversation-context-composer".to_owned(),
    MeterPlace::Bar,
    Some(Over(over.clone())),
  );
  meter(ctx, project, (run, step), place)
}

fn meter(
  ctx: &mut Ctx,
  project: &ShownProject,
  (run, step): (&RunView, &StepView),
  (id, place, over): (String, MeterPlace, Option<Over>),
) -> Option<Element> {
  let execution = step.execution?;
  let props = StepContextProps {
    project: project.clone(),
    run: run.id(),
    execution,
    meter: ContextMeterProps {
      id: id.as_str().into(),
      reading: step.context,
      agent: step.agent.clone(),
      model: step.model.clone(),
      place,
      over,
      // A run step's session is not compacted from here in this build.
      compact: Compact::NotOffered,
    },
  };
  // Keyed by the execution it reads: another execution's meter (the next step's, a retry's) starts afresh from the
  // reading the task was read with, never from the reading this one last read.
  Some(ctx.mount_keyed::<StepContext>(&format!("{id}:{execution}"), props))
}

#[derive(Clone, PartialEq)]
struct StepContextProps {
  project: ShownProject,
  run: RunId,
  execution: StepExecutionId,
  /// The meter as the task was read: its reading is the one the task was read with.
  meter: ContextMeterProps,
}

impl DevtoolsInspectable for StepContextProps {}

struct StepContext;

impl Component for StepContext {
  type Props = StepContextProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StepContextProps>().clone();
    // Every change of the run, its transcript's lines too: a turn's reading comes with them. Other runs' do not.
    let changes = run_changes::of_run(ctx, &props.project, Some(props.run));
    let root: PathBuf = props.project.open.repo_root.clone();
    let hold = Services::of(ctx).hold;
    let state = settled_read(
      ctx,
      "context use",
      (root, props.run, props.execution, changes),
      move |(root, run, execution, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || of_execution_in(&root, run, execution).map(MeterReading))
            .await
            .and_then(|read| read)
        }
      },
    );
    // A read that failed keeps the reading last shown: the transcript's own read says when the run cannot be read.
    let reading: Option<ContextUse> = state.data.map_or(props.meter.reading, |MeterReading(reading)| reading);
    context_meter(ctx, ContextMeterProps { reading, ..props.meter })
  }
}
