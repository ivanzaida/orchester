//! *Open full log*: every line Orchester keeps of the service, oldest first, in a modal over the page, read from the
//! runner as it opens; its secret values were replaced before the runner kept a line. The board draws no frame for it
//! (`Kd8zD` is named in `docs/architecture/agent-testing.md` but not drawn): it is the app's dialog with the panel's
//! lines in a box that scrolls.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_ipc::services::{MAX_SHOWN_LINES, ServiceCommand, ServiceLogLine, ServiceReply};
use orchester_workspace::{
  messages::{self, Message},
  tasks::run::control_message,
};

use super::log::{LogProps, line, log_box};
use crate::{
  services::Services,
  theme::RunServicesText,
  ui::{
    dialog::{Dialog, Tone},
    icon::Icon,
    message::message_text,
  },
};

/// The dialog's width, as wide as a lifecycle dialog for a log's long lines.
const WIDTH: f32 = 880.0;
/// The most the lines take before they scroll.
const LINES_HEIGHT: f32 = 480.0;

/// The kept lines, as read when the dialog opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Kept(Vec<ServiceLogLine>);

impl lurq::app::component::DevtoolsInspectable for Kept {}

/// The dialog while `open` holds.
pub(super) fn dialog(ctx: &mut Ctx, props: &LogProps, open: &Signal<bool>) -> Option<Element> {
  let shown = open.get();
  let runners = Services::of(ctx).runners;
  let deps = (props.root.clone(), props.run, props.service.clone(), shown);
  let read = ctx.future(deps, move |(root, run, service, shown)| {
    let runners = runners.clone();
    async move {
      if !shown {
        return Ok(Kept::default());
      }
      let command = ServiceCommand::Log {
        run,
        service: service.to_string(),
        since: 0,
        tail: MAX_SHOWN_LINES,
      };
      match runners.services(&root, command).await {
        Ok(ServiceReply::Log { lines, .. }) => Ok(Kept(lines)),
        Ok(_) => Err(messages::interrupted()),
        Err(error) => Err(control_message(&error)),
      }
    }
  });
  if !shown {
    return None;
  }
  let state = read.state().get();
  let body: Element = match (state.data, state.error) {
    (_, Some(error)) => failed(ctx, &error),
    (Some(Kept(lines)), None) => {
      let lines = log_box("task-services-full-log-lines").with_children(lines.iter().map(line));
      ScrollVertical::new(lines)
        .id("task-services-full-log-scroll")
        .width(Dimension::Pct(100.0))
        .max_height(Dimension::Px(LINES_HEIGHT))
        .into()
    }
    (None, None) => log_box("task-services-full-log-lines").into(),
  };
  let name = props.service.to_string();
  Some(
    Dialog::new(
      "task-services-full-log",
      Icon::ScrollText,
      ctx.t_args("run_services.log.fullTitle", [("name", name.as_str())]),
      ctx.t("run_services.log.close"),
    )
    .tone(Tone::Neutral)
    .width(WIDTH)
    .what(ctx.t_args("run_services.log.fullWhat", [("name", name.as_str())]))
    .part(body)
    .plain_footer()
    .render(ctx, open),
  )
}

fn failed(ctx: &Ctx, error: &Message) -> Element {
  Text::new(&message_text(ctx, error))
    .id("task-services-full-log-failed")
    .variant(RunServicesText::LogLine.role())
    .color(PaletteColor::TextMuted)
    .into()
}
