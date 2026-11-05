//! The selected service's live log under its row (`Log panel` `iw8mT`): on `hover`, 14 at its sides and 12 under it,
//! its parts 8 apart. Its head is a 13 `text_muted` `scroll-text`, `team-server · live log` in 12/500
//! `text_secondary` and how many lines it shows in 12 `text_muted`, 10 apart, then *Follow* and *Open full log* at its
//! end (`WGv2K`). Its last lines follow on `code_bg` in a `border` stroke, radius 6, 8 over and under and 10 at the
//! sides, 3 apart (`a8D1Ia`): when each came in Geist Mono 11.5 `text_muted`, 10 before what it says in Geist Mono 11.5
//! `text_secondary`, standard error's in `danger`.
//!
//! While *Follow* is on, the runner is asked every second for the lines after the last one read; off, the log holds
//! still. The runner replaced the service's secret values before it kept a line, so none reaches the tab.

use std::{
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_ipc::services::{ServiceCommand, ServiceLogLine, ServiceReply};
use orchester_workspace::{display, messages::Message, tasks::run::control_message};

use super::full_log;
use crate::{
  services::Services,
  theme::{ExtraColor, ExtraSpacing, IconSize, RunServicesText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    toggle::{Toggle, ToggleKind},
  },
};

/// How often the runner is asked while the log follows.
const POLL: Duration = Duration::from_secs(1);
/// The lines the panel shows, the latest; *Open full log* shows every line kept.
const SHOWN_LINES: usize = 12;

/// The service whose log the panel shows: its name, and when it started, so a restart starts a log of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LogProps {
  pub root: PathBuf,
  pub run: RunId,
  pub service: Arc<str>,
  pub started_at: Timestamp,
}

impl DevtoolsInspectable for LogProps {}

/// The latest lines read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Lines(Arc<[ServiceLogLine]>);

impl DevtoolsInspectable for Lines {}

pub(super) struct LogPanel {
  follow: Signal<bool>,
  /// What the reader of the lines reads `follow` by.
  following: Arc<AtomicBool>,
  full: Signal<bool>,
}

impl Component for LogPanel {
  type Props = LogProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      follow: ctx.signal(true),
      following: Arc::new(AtomicBool::new(true)),
      full: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LogProps>().clone();
    let read = self.read(ctx, &props);
    let state = read.state().get();
    let lines = state.data.unwrap_or_default().0;
    let head = self.head(ctx, &props, lines.len());
    let tail = match (&state.error, lines.is_empty()) {
      (Some(_), true) => failed(ctx, &props),
      _ => tail(&lines),
    };
    Column::new()
      .id("task-services-log")
      .describe("service", props.service.to_string())
      .describe("following", if self.follow.get() { "true" } else { "false" })
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(ExtraSpacing::Inset.role())
          .right(ExtraSpacing::Inset.role())
          .bottom(SpacingSize::Md),
      )
      .spacing(SpacingSize::Sm)
      .background(ExtraColor::Hover.role())
      .child(head)
      .child(tail)
      .with_children(full_log::dialog(ctx, &props, &self.full))
  }
}

impl LogPanel {
  /// Reads the service's lines, the ones after the last read every second while the log follows.
  fn read(&self, ctx: &mut Ctx, props: &LogProps) -> lurq::app::ctx::StreamHandle<Lines, Message> {
    let runners = Services::of(ctx).runners;
    let following = Arc::clone(&self.following);
    let deps = (props.root.clone(), props.run, props.service.clone(), props.started_at);
    ctx.stream(deps, move |(root, run, service, _), emitter: StreamEmitter<Lines, Message>| {
      let (runners, following) = (runners.clone(), Arc::clone(&following));
      async move {
        let mut kept: Vec<ServiceLogLine> = Vec::new();
        let mut since = 0;
        loop {
          if following.load(Ordering::Acquire) {
            let command = ServiceCommand::Log {
              run,
              service: service.to_string(),
              since,
              tail: u32::try_from(SHOWN_LINES).expect("a few lines"),
            };
            match runners.services(&root, command).await {
              Ok(ServiceReply::Log { lines, last }) => {
                since = last;
                if !lines.is_empty() || kept.is_empty() {
                  kept.extend(lines);
                  kept.drain(..kept.len().saturating_sub(SHOWN_LINES));
                  if !emitter.emit(Lines(kept.clone().into())) {
                    return;
                  }
                }
              }
              Ok(other) => tracing::error!(?other, "unexpected answer to a service's log"),
              Err(error) => {
                tracing::debug!(%error, "a service's log cannot be read now");
                if !emitter.reject(control_message(&error)) {
                  return;
                }
              }
            }
          }
          tokio::time::sleep(POLL).await;
        }
      }
    })
  }

  /// The head: what the log is, how many lines it shows, *Follow* and *Open full log*.
  fn head(&self, ctx: &Ctx, props: &LogProps, count: usize) -> Row {
    let shown = match count {
      0 => ctx.t("run_services.log.count.none"),
      1 => ctx.t("run_services.log.count.one"),
      count => ctx.t_args("run_services.log.count.other", [("count", count.to_string())]),
    };
    let (follow, following) = (self.follow.clone(), Arc::clone(&self.following));
    let toggle = Toggle::new(
      "task-services-log-follow",
      ToggleKind::Checkbox,
      self.follow.get(),
      ctx.t("run_services.log.follow"),
    )
    .on_change(move |on| {
      following.store(on, Ordering::Release);
      follow.set(on);
    });
    let full = self.full.clone();
    let open_full = ActionButton::new(
      "task-services-log-full",
      ButtonKind::Ghost,
      ctx.t("run_services.log.openFull"),
    )
    .icon(Icon::ArrowUpRight)
    .small()
    .on_click(move || full.set(true));
    Row::new()
      .id("task-services-log-head")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::ScrollText, IconSize::Base, PaletteColor::TextMuted))
      .child(
        Text::new(&ctx.t_args("run_services.log.title", [("name", props.service.to_string())]))
          .id("task-services-log-title")
          .variant(RunServicesText::LogTitle.role())
          .nowrap(),
      )
      .child(
        Text::new(&shown)
          .id("task-services-log-count")
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
      .child(Spacer::new().flex(1.0))
      .child(toggle)
      .child(open_full)
  }
}

/// The lines on `code_bg`.
pub(super) fn tail(lines: &[ServiceLogLine]) -> Element {
  log_box("task-services-log-lines").with_children(lines.iter().map(line)).into()
}

/// The box the lines stand in.
pub(super) fn log_box(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Fine.role())
    .background(ExtraColor::CodeBackground.role())
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
}

/// One line: when it came, and what it says.
pub(super) fn line(line: &ServiceLogLine) -> Element {
  let text = Text::new(&line.text)
    .id(format!("task-services-log-line-{}", line.number))
    .variant(RunServicesText::LogLine.role())
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  let text = if line.stderr {
    text.color(PaletteColor::Danger)
  } else {
    text
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .describe("stream", if line.stderr { "err" } else { "out" })
    .child(
      Text::new(&display::clock_seconds(line.at))
        .variant(RunServicesText::LogTime.role())
        .nowrap(),
    )
    .child(text)
    .into()
}

/// Why the log could not be read.
fn failed(ctx: &Ctx, props: &LogProps) -> Element {
  log_box("task-services-log-lines")
    .child(
      Text::new(&ctx.t_args("run_services.log.failed", [("name", props.service.to_string())]))
        .id("task-services-log-failed")
        .variant(RunServicesText::LogLine.role())
        .color(PaletteColor::TextMuted),
    )
    .into()
}
