//! The Delivery view beside Usage (ORC-50; no frames yet: laid out with the Usage page's header, cards, charts and
//! tables): when the project's queued work and open modules are likely delivered ([`forecast`]), then *Throughput*
//! beside *Cycle time*, and *Where time goes* beside *Cumulative flow*, over the last 30, 90 or 365 days. Every chart
//! reads as a table too. What counts and how is `orchester_workspace::delivery`; it is read off the UI thread, again
//! whenever the project's runs or tasks change and when the period changes.

mod bars;
mod cycle;
mod flow;
mod flow_chart;
mod forecast;
mod throughput;
mod time;
mod toolbar;
pub(crate) mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
    theme::SpacingSize,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{time::Timestamp, usage::local_date};
use orchester_workspace::{
  background,
  delivery::{Delivery, DeliveryPeriod},
  messages::{self, Message},
};

pub(crate) use self::toolbar::view_switch;
use crate::{
  project::ShownProject,
  screens::usage::card::{card_row, footnote},
  theme::UsageSpacing,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, period: DeliveryPeriod) -> Element {
  let key = format!("delivery-{}", project.open.project.id());
  ctx.mount_keyed::<DeliveryScreen>(
    &key,
    DeliveryProps {
      project: project.clone(),
      period,
    },
  )
}

#[derive(Clone, PartialEq)]
pub struct DeliveryProps {
  project: ShownProject,
  period: DeliveryPeriod,
}

impl DevtoolsInspectable for DeliveryProps {}

/// Which charts read as tables, which the view keeps while its period changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Views {
  pub throughput: bool,
  pub cycle: bool,
  pub time: bool,
  pub flow: bool,
}

impl DevtoolsInspectable for Views {}

/// The delivery as read.
#[derive(Clone)]
pub struct Read(Arc<Delivery>);

impl PartialEq for Read {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Read {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadFailed(Message);

impl DevtoolsInspectable for ReadFailed {}

pub struct DeliveryScreen {
  views: Signal<Views>,
  retry: Signal<u64>,
}

impl Component for DeliveryScreen {
  type Props = DeliveryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      views: ctx.signal(Views::default()),
      retry: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let DeliveryProps { project, period } = ctx.props::<DeliveryProps>().clone();
    let id = project.open.project.id();
    let name = project.open.project.name().as_str().to_owned();
    // Again whenever the project's runs or tasks change, or Retry is pressed.
    let deps = (
      project.feed.changes.get(),
      project.feed.revision.get(),
      self.retry.get(),
    );
    let read = read(ctx, &project, period, deps);
    let failed = read.error.as_ref().filter(|_| read.is_rejected());
    let content = match (failed, &read.data) {
      (Some(ReadFailed(message)), _) => {
        let retry = self.retry.clone();
        let body = message_text(ctx, message);
        vec![load_error(
          ctx,
          "delivery-load-error",
          (&ctx.t("delivery.readFailed"), &body),
          move || retry.update(|count| *count += 1),
        )]
      }
      (None, Some(Read(delivery))) => self.sections(ctx, id, delivery),
      (None, None) => vec![loading("delivery-loading", &ctx.t("delivery.reading")).into()],
    };
    Column::new()
      // The Delivery view belongs to the Usage screen, as a module's page does to Modules.
      .id("screen-usage")
      .describe("view", "delivery")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(toolbar::toolbar(ctx, (id, &name), period))
      .child(
        ScrollVertical::new(
          Column::new()
            .id("delivery-sections")
            .width(Dimension::Pct(100.0))
            .padding(
              Padding::new()
                .top(SpacingSize::Xl)
                .bottom(SpacingSize::Section)
                .left(UsageSpacing::Page.role())
                .right(UsageSpacing::Page.role()),
            )
            .spacing(SpacingSize::Xl)
            .with_children(content),
        )
        .id("delivery-scroll")
        .width(Dimension::Pct(100.0))
        .flex(1.0),
      )
  }
}

impl DeliveryScreen {
  fn sections(&self, ctx: &mut Ctx, project: orchester_core::id::ProjectId, delivery: &Delivery) -> Vec<Element> {
    let mut sections: Vec<Element> = unread_runs(ctx, delivery.unread_runs).into_iter().collect();
    sections.push(forecast::card(ctx, project, delivery));
    let throughput = throughput::card(ctx, delivery, &self.views);
    let cycle = cycle::card(ctx, &delivery.cycle, &self.views);
    sections.push(card_row("delivery-row-flow", vec![throughput, cycle]));
    let time = time::card(ctx, &delivery.time, &self.views);
    let flow = flow::card(ctx, &delivery.flow, &self.views);
    sections.push(card_row("delivery-row-time", vec![time, flow]));
    sections
  }
}

/// That runs whose record is damaged are left out, never unsaid.
fn unread_runs(ctx: &Ctx, count: usize) -> Option<Element> {
  let text = match count {
    0 => return None,
    1 => ctx.t("delivery.unreadRuns.one"),
    count => ctx.t_args("delivery.unreadRuns.other", [("count", count.to_string().as_str())]),
  };
  Some(footnote("delivery-unread-runs", &text))
}

/// The delivery of `project` over `period`, read off the UI thread again whenever `deps` change.
fn read<D>(ctx: &mut Ctx, project: &ShownProject, period: DeliveryPeriod, deps: D) -> FutureState<Read, ReadFailed>
where
  D: Clone + PartialEq + Send + Sync + 'static,
{
  let root = project.open.repo_root.clone();
  ctx
    .future((root, period, deps), |(root, period, _)| async move {
      background::run(move || {
        let now = Timestamp::now();
        let db = orchester_store::ProjectDb::open_read_only(&root)?;
        Delivery::read(&db, period.range(local_date(now)), now)
      })
      .await
      .map_err(ReadFailed)?
      .map(|delivery| Read(Arc::new(delivery)))
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project's delivery");
        ReadFailed(messages::store_error(&error))
      })
    })
    .state()
    .get()
}
