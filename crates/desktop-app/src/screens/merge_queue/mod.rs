//! The project's Merge queue (ORC-19, `Merge queue · dark` `seRYH`, blocked `H7OHPT`, empty `mtrlT`, row states
//! `vM4TB`): runs of pipelines with a Deliver step, delivered into their base branch one at a time
//! (`orchester_workspace::merge_queue`). Under its head and the base box, the run delivering now, the runs waiting
//! their turn with their order, those waiting for approval, which do not hold the queue, and what was merged or left
//! the queue today. While delivery is blocked a notice says why. An empty queue says how each pipeline delivers.
//! The queue is read again whenever it changes, the project's runs change state, or the person tries again.

mod actions;
mod empty;
mod header;
pub(crate) mod phases;
mod reading;
pub(crate) mod row;
pub(crate) mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::merge_queue::QueueState;
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
  theme::{DeliverySpacing, DeliveryText, ExtraSpacing, ExtraTypography},
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
          .top(ExtraSpacing::Roomy.role())
          .bottom(SpacingSize::Xl)
          .left(DeliverySpacing::Page.role())
          .right(DeliverySpacing::Page.role()),
      )
      .spacing(DeliverySpacing::Sections.role());
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
      held: read
        .rows
        .iter()
        .any(|row| matches!(row.entry.state, QueueState::Blocked(_))),
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
      .child(header::header(ctx, &read, &checkout))
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
                .spacing(DeliverySpacing::Sections.role())
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

/// The queue's sections, or the empty state and today's section.
fn body(ctx: &mut Ctx, read: &QueueRead, rows: &Rows) -> Vec<Element> {
  let of = |shown: fn(&QueueState) -> bool| -> Vec<&QueueRow> {
    read.rows.iter().filter(|row| shown(&row.entry.state)).collect()
  };
  let delivering = of(QueueState::delivering);
  let waiting = of(|state| *state == QueueState::Queued);
  let approval = of(|state| *state == QueueState::WaitingForApproval);
  let today = of(|state| matches!(state, QueueState::Merged(_) | QueueState::Left(_)));
  let mut body = Vec::new();
  if read.in_queue() == 0 {
    body.push(empty::empty(ctx, rows.project, &read.pipelines).into());
  }
  if !delivering.is_empty() {
    let blocked = delivering
      .iter()
      .any(|row| matches!(row.entry.state, QueueState::Blocked(_)));
    let head = Head::new(ctx.t("delivery.queue.section.delivering"));
    body.push(section(ctx, ("delivering", head, blocked), &delivering, rows));
  }
  if !waiting.is_empty() {
    let head = Head::new(ctx.t("delivery.queue.section.waiting"))
      .count(waiting.len())
      .note(ctx.t("delivery.queue.section.waitingNote"));
    body.push(section(ctx, ("waiting", head, false), &waiting, rows));
  }
  if !approval.is_empty() {
    let head = Head::new(ctx.t("delivery.queue.section.approval"))
      .count(approval.len())
      .note(ctx.t("delivery.queue.section.approvalNote"));
    body.push(section(ctx, ("approval", head, false), &approval, rows));
  }
  body.push(today_section(ctx, &today, rows));
  body
}

fn today_section(ctx: &mut Ctx, today: &[&QueueRow], rows: &Rows) -> Element {
  let merged = today
    .iter()
    .filter(|row| matches!(row.entry.state, QueueState::Merged(_)))
    .count();
  let mut head = Head::new(ctx.t("delivery.queue.section.today")).count(today.len());
  if !today.is_empty() {
    head = head.note(ctx.t_args(
      "delivery.queue.section.todayNote",
      [
        ("merged", merged.to_string()),
        ("failed", (today.len() - merged).to_string()),
      ],
    ));
  }
  if today.is_empty() {
    let nothing = Row::new()
      .id("merge-queue-today-nothing")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
      .child(
        Text::new(&ctx.t("delivery.queue.section.nothingToday"))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      );
    return section_frame(ctx, ("today", head, false), vec![nothing.into()]);
  }
  section(ctx, ("today", head, false), today, rows)
}

/// A section's head: its title, the count of its runs and what it notes.
struct Head {
  title: Arc<str>,
  count: Option<usize>,
  note: Option<Arc<str>>,
}

impl Head {
  const fn new(title: Arc<str>) -> Self {
    Self {
      title,
      count: None,
      note: None,
    }
  }

  const fn count(mut self, count: usize) -> Self {
    self.count = Some(count);
    self
  }

  fn note(mut self, note: Arc<str>) -> Self {
    self.note = Some(note);
    self
  }
}

fn section(ctx: &mut Ctx, (name, head, blocked): (&str, Head, bool), shown: &[&QueueRow], rows: &Rows) -> Element {
  let children = shown.iter().map(|shown| row::row(ctx, rows, shown)).collect();
  section_frame(ctx, (name, head, blocked), children)
}

/// The head over the rows' box: `surface_panel` in `border`, radius 10, `warning` while the run in it is blocked.
fn section_frame(_ctx: &Ctx, (name, head, blocked): (&str, Head, bool), children: Vec<Element>) -> Element {
  let stroke = if blocked {
    PaletteColor::Warning
  } else {
    PaletteColor::Border
  };
  Column::new()
    .id(format!("merge-queue-{name}"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&head.title).variant(ExtraTypography::FormSection.role()))
        .with_children(head.count.map(|count| {
          Text::new(&count.to_string())
            .id(format!("merge-queue-{name}-count"))
            .variant(DeliveryText::Count.role())
        }))
        .with_children(head.note.map(|note| {
          Text::new(&note)
            .variant(ExtraTypography::Prose.role())
            .color(PaletteColor::TextMuted)
        })),
    )
    .child(
      Column::new()
        .id(format!("merge-queue-{name}-rows"))
        .width(Dimension::Pct(100.0))
        .background(PaletteColor::SurfacePanel)
        .border(Border::inside(BorderSize::Sm, stroke))
        .rounded(RadiusSize::Lg)
        .with_children(children),
    )
    .into()
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
