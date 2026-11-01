//! *Also needed · set for this computer* (`PVOJl` in `S03`): *Let automation answer approvals*, which answering agents
//! also needs, read-only with its state and *Open*, which goes to Settings › Automation where it is set once for every
//! project. Read off the UI thread when the tab shows.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureState},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use super::super::{
  TabProps,
  kit::{Group, SettingRow, value},
};
use crate::{
  navigation::Navigation,
  routes::Route,
  services::Services,
  theme::{ExtraSpacing, IconSize, ProjectSettingsText},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::opening,
    skeleton,
  },
};

const ID: &str = "settings-orchestrator-answer-approvals";
/// The bone of the state while it is read, as wide as *Off* (`Value` `HKkDT`).
const VALUE_BONE: f32 = 18.0;

pub(super) struct ThisComputer;

impl Component for ThisComputer {
  type Props = TabProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let read = answers_approvals(ctx);
    let state: Element = match read.data {
      Some(on) => value(
        &format!("{ID}-value"),
        &ctx.t(if on {
          "project_settings.orchestrator.answerApprovals.on"
        } else {
          "project_settings.orchestrator.answerApprovals.off"
        }),
      )
      .into(),
      None => skeleton::line(ctx, ProjectSettingsText::Value.role(), VALUE_BONE).into(),
    };
    let problem = read.error.map(|reason| {
      let reason = message_text(ctx, &reason);
      Element::from(field_error(
        &format!("{ID}-error"),
        &ctx.t_args(
          "project_settings.orchestrator.answerApprovals.unreadable",
          [("reason", &*reason)],
        ),
      ))
    });
    // A failed read leaves no state to show, only why under the row.
    let state = (problem.is_none()).then_some(state);
    let control = Row::new()
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .with_children(state)
      .child(open(ctx));
    let row = SettingRow::new(ID, ctx.t("project_settings.orchestrator.answerApprovals.label"))
      .hint(ctx.t("project_settings.orchestrator.answerApprovals.hint"))
      .control(control)
      .under_all(problem)
      .render(ctx);
    Element::from(
      Group::new(
        "settings-orchestrator-computer",
        ctx.t("project_settings.orchestrator.alsoNeeded"),
      )
      .row(row)
      .foot(&ctx.t("project_settings.orchestrator.alsoNeededFoot")),
    )
  }
}

/// *Open* and its arrow (`Link` `JmCR1`): Settings › Automation, where the setting is set.
fn open(ctx: &mut Ctx) -> Element {
  let words = ctx.t("project_settings.orchestrator.answerApprovals.open");
  opening(Navigation::of(ctx).to(Route::Settings))
    .id(format!("{ID}-open"))
    .describe("a11y_name", &*words)
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Text::new(&words)
        .id(format!("{ID}-open-text"))
        .variant(ProjectSettingsText::Value.role())
        .color(PaletteColor::TextPrimary),
    )
    .child(icon(Icon::ArrowRight, IconSize::Base, PaletteColor::TextPrimary))
    .into()
}

/// Whether automation may answer approvals on this computer, as stored.
fn answers_approvals(ctx: &mut Ctx) -> FutureState<bool, Message> {
  let services = Services::of(ctx);
  let (workspace, hold) = (services.workspace, services.hold);
  ctx
    .future((), move |()| {
      let (workspace, hold) = (workspace.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || workspace.automation_answers_approvals())
          .await?
          .map_err(|error| {
            tracing::error!(%error, "Let automation answer approvals cannot be read");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
}
