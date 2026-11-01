//! The project's Merge queue (ORC-19; a lane per base branch, ORC-410, 05 `S1pTL` of `Base branch · redesign`): runs
//! of pipelines with a Deliver step, delivered into their own base branch one at a time per branch, first finished
//! first (`orchester_workspace::merge_queue`). Under the info line with how the checkout stands, a lane per base branch
//! a run of the queue delivers into, once one joined: its head says how its runs land, its box holds its runs in the
//! queue and those merged or out of it today. What holds one lane holds no other; the note under the lanes says so.
//! While delivery is blocked a notice says why. An empty queue says how each pipeline delivers. The queue is read again
//! whenever it changes, the project's runs change state, or the person tries again.

mod actions;
mod empty;
mod header;
mod lane;
pub(crate) mod phases;
mod reading;
mod row;
pub(crate) mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  background,
  display::plain_path,
  merge_queue::{self, QueueRead, QueueRow},
  messages::{self, Message},
};

use self::{actions::QueueActions, row::Rows};
use crate::{
  project::ShownProject,
  services::Services,
  theme::{DeliverySpacing, HubSpacing},
  ui::{
    message::message_text,
    notice::{Notice, NoticeTone},
    state::load_error,
  },
};

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
}

impl DevtoolsInspectable for Props {}

/// The Merge queue of `project`.
pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<MergeQueueScreen>(
    &format!("merge-queue-{}", project.open.project.id()),
    Props {
      project: project.clone(),
    },
  )
}

struct MergeQueueScreen {
  retries: Signal<u64>,
  /// Why the last change a button asked for did not happen.
  failed: Signal<Option<Message>>,
}

impl Component for MergeQueueScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retries: ctx.signal(0),
      failed: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project } = ctx.props::<Props>().clone();
    let root = project.open.repo_root.clone();
    let frame = Column::new()
      .id("screen-merge_queue")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(DeliverySpacing::Lanes.role())
          .bottom(SpacingSize::Xl)
          .left(HubSpacing::Side.role())
          .right(HubSpacing::Side.role()),
      )
      .spacing(DeliverySpacing::Lanes.role());
    let read = match read(ctx, &project, self.retries.get()) {
      Read::Reading => return Element::from(frame.child(reading::page(ctx))),
      Read::Failed(reason) => {
        let retries = self.retries.clone();
        let body = ctx.t_args(
          "delivery.queue.readFailedBody",
          [("reason", message_text(ctx, &reason))],
        );
        return frame
          .child(load_error(
            ctx,
            "merge-queue-load-error",
            (&ctx.t("delivery.queue.readFailed"), &body),
            move || retries.update(|count| *count += 1),
          ))
          .into();
      }
      Read::Ready(read) => read,
    };
    let checkout: Arc<str> = plain_path(&root.display().to_string()).into();
    let actions = QueueActions::new(&root, self.failed.clone());
    let rows = Rows {
      project: project.open.project.id(),
      checkout: checkout.clone(),
      actions: actions.clone(),
    };
    let failed = self.failed.get().map(|reason| {
      Notice::new(
        "merge-queue-not-done",
        NoticeTone::Danger,
        ctx.t_args("delivery.queue.notDone", [("reason", message_text(ctx, &reason))]),
      )
    });
    let body = body(ctx, &read, &rows);
    frame
      .child(header::header(ctx, &read))
      .with_children(header::blocked(ctx, &read, (&root, &checkout), &actions))
      .with_children(failed)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(
            ScrollVertical::new(
              Column::new()
                .width(Dimension::Pct(100.0))
                .padding(Padding::new().right(SpacingSize::Sm))
                .spacing(DeliverySpacing::Lanes.role())
                .with_children(body),
            )
            .id("merge-queue-main")
            .width(Dimension::Pct(100.0))
            .height(Dimension::Pct(100.0)),
          ),
      )
      .into()
  }
}

/// The empty state while no run is in the queue; then a lane per base branch, in the order of their first run, the
/// runs that recorded no base branch after them, and the note.
fn body(ctx: &mut Ctx, read: &QueueRead, rows: &Rows) -> Vec<Element> {
  let mut body = Vec::new();
  if read.in_queue() == 0 {
    body.push(empty::empty(ctx, rows.project, &read.pipelines).into());
  }
  for shown in &read.lanes {
    body.push(lane::lane(ctx, rows, read, shown));
  }
  let without: Vec<&QueueRow> = read.rows.iter().filter(|row| row.base_branch.is_none()).collect();
  if !without.is_empty() {
    body.push(lane::without_lane(ctx, rows, &without));
  }
  if !read.lanes.is_empty() {
    body.push(lane::note(ctx, read).into());
  }
  body
}

/// What the screen shows of the queue.
#[derive(Clone, Debug, PartialEq)]
enum Read {
  Reading,
  Ready(QueueRead),
  Failed(Message),
}

fn read(ctx: &mut Ctx, project: &ShownProject, retries: u64) -> Read {
  let root = project.open.repo_root.clone();
  let changes = crate::project::merge_queue::changes(ctx, &root);
  let again = changes + project.feed.states.get() + project.feed.revision.get() + retries;
  let services = Services::of(ctx);
  let (programs, hold) = (services.programs, services.hold);
  let loaded = ctx
    .future((root, again), move |(root, _)| {
      let (programs, hold) = (programs.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || merge_queue::read(&root, &programs))
          .await?
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get();
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  loaded.data.map_or(Read::Reading, Read::Ready)
}
