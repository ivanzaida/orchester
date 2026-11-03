//! Frame 02's Test area (i2ygEE). Only the button submits recorded requests.
mod actions;
mod table;
mod words;

use std::time::Duration;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::DecisionConnection,
  id::IdempotencyKey,
  run::review::{DecisionTestStatus, ReviewerChoice},
};
use orchester_workspace::{
  background,
  connections::decision::{Writer, test},
  messages,
};

use crate::{
  project::ShownProject,
  screens::project_settings::kit::Group,
  theme::{ConnectionsText, ExtraSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub connection: DecisionConnection,
  pub reviewer: Option<ReviewerChoice>,
}
impl DevtoolsInspectable for Props {}

#[derive(Clone, Debug, PartialEq)]
struct Read(Option<orchester_core::run::review::DecisionTestReport>);
impl DevtoolsInspectable for Read {}
pub(super) struct TestSection {
  start: actions::Action,
  stop: actions::Stop,
  revision: Signal<u64>,
  poll: Interval,
}

impl Component for TestSection {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let revision = ctx.signal(0_u64);
    let next = revision.clone();
    let poll = ctx.create_interval(Duration::from_millis(500), move || next.update(|n| *n += 1));
    let start = actions::start(ctx);
    let next = revision.clone();
    ctx.watch(&start.state(), move |state| {
      if !state.is_pending() {
        next.update(|n| *n += 1);
      }
    });
    Self {
      start,
      stop: actions::stop(ctx),
      revision,
      poll,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let connection = props.connection.clone();
    let read = ctx
      .future(
        (root, connection, self.revision.get()),
        |(root, connection, _)| async move {
          background::run(move || test::latest_in(&root, connection.id()))
            .await?
            .map(Read)
            .map_err(|error| messages::store_error(&error))
        },
      )
      .state()
      .get();
    let report = read
      .data
      .as_ref()
      .and_then(|report| report.0.as_ref())
      .filter(|r| r.base_url == *props.connection.base_url() && r.key == props.connection.key());
    let active = report.is_some_and(|r| r.status == DecisionTestStatus::Running);
    let pending = self.start.state().get().is_pending();
    if active || pending {
      self.poll.start();
    } else {
      self.poll.stop();
    }
    let error = read.error.as_ref().or(self.start.state().get().error.as_ref()).cloned();
    let stop_error = self.stop.state().get().error;
    let mut group = Group::new("decision-test", ctx.t("decision_connection.testTitle"))
      .after(words::lead(ctx))
      .after(table::render(ctx, report, props.reviewer.as_ref()))
      .after(self.footer(
        ctx,
        &props,
        report,
        pending || active,
        read.is_fulfilled() && read.error.is_none(),
      ));
    if let Some(error) = error.or(stop_error) {
      group = group.after(field_error("decision-test-error", &message_text(ctx, &error)));
    }
    if read.error.is_some() {
      let revision = self.revision.clone();
      group = group.after(
        ActionButton::new(
          "decision-test-retry",
          ButtonKind::Secondary,
          ctx.t("decision_connection.testRetry"),
        )
        .on_click(move || revision.update(|n| *n += 1)),
      );
    }
    if let Some(note) = report.and_then(|report| words::caveat(ctx, report)) {
      group = group.after(note);
    }
    Element::from(group.after(words::sent(ctx, &props.connection)))
  }
}

impl TestSection {
  fn footer(
    &self,
    ctx: &Ctx,
    props: &Props,
    report: Option<&orchester_core::run::review::DecisionTestReport>,
    running: bool,
    readable: bool,
  ) -> Element {
    let meta = if report.is_none() && !running && !readable {
      ctx.t("decision_connection.testReading")
    } else if report.is_none() && !running {
      let model = props
        .reviewer
        .as_ref()
        .map(|r| &r.model)
        .or(props.connection.models().first());
      model.map_or_else(
        || words::meta(ctx, report, running),
        |model| ctx.t_args("decision_connection.testEmptyModel", [("model", model.as_str())]),
      )
    } else {
      words::meta(ctx, report, running)
    };
    let button = if running {
      let stop = self.stop.clone();
      let root = props.project.open.repo_root.clone();
      let id = report.filter(|r| r.status == DecisionTestStatus::Running).map(|r| r.id);
      let button = ActionButton::new(
        "decision-test-stop",
        ButtonKind::Secondary,
        ctx.t("decision_connection.testStop"),
      )
      .icon(Icon::Square);
      if let Some(id) = id {
        let button = button.on_click(move || stop.run((root.clone(), id)));
        if self.stop.state().get().is_pending() {
          button.loading()
        } else {
          button
        }
      } else {
        button.disabled()
      }
    } else {
      self.test_button(ctx, props, report.is_some(), readable)
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&meta)
          .id("decision-test-meta")
          .variant(ConnectionsText::CompactLine.role())
          .flex(1.0),
      )
      .child(button)
      .into()
  }

  fn test_button(&self, ctx: &Ctx, props: &Props, tested: bool, readable: bool) -> ActionButton {
    let button = ActionButton::new(
      "decision-test-start",
      ButtonKind::Secondary,
      ctx.t(if tested {
        "decision_connection.testAgain"
      } else {
        "decision_connection.testStart"
      }),
    )
    .icon(Icon::Play);
    if !readable {
      return button.disabled();
    }
    let model = props
      .reviewer
      .as_ref()
      .map(|r| &r.model)
      .or(props.connection.models().first());
    let Some(model) = model.filter(|m| props.connection.models().contains(m)).cloned() else {
      return button.unavailable(ctx.t("decision_connection.testNoModel"));
    };
    let (start, root, connection) = (
      self.start.clone(),
      props.project.open.repo_root.clone(),
      props.connection.clone(),
    );
    button.on_click(move || {
      if start.state().get_untracked().is_pending() {
        return;
      }
      let writer = if lurq::app::events::is_synthetic_input() {
        Writer::Automation
      } else {
        Writer::Person
      };
      start.run((
        root.clone(),
        IdempotencyKey::generate(),
        connection.clone(),
        model.clone(),
        writer,
      ));
    })
  }
}
