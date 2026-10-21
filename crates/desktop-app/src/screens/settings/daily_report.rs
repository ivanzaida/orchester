//! *Daily report from the Orchestrator* (ORC-47) under *What automation may do*: a switch row (`Form row` `XaNIz`) that
//! is off until a person turns it on, and while it is on, the time of day the report comes at, in the quiet hours'
//! time select (`super::notifications`). No design frame draws it yet. The setting is read once when the section
//! opens and saved off the UI thread at each change; until it is read the switch stays as it is, and a setting that
//! cannot be read or was not saved says why under the row.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::orchestrator::ReportTime;
use orchester_workspace::{
  background,
  messages::{self, Message},
};

use super::{
  automation::row,
  notifications::{TIME_WIDTH, clock, minute, show, time_options},
};
use crate::{
  services::Services,
  theme::ExtraSpacing,
  ui::{
    field::{SelectField, field_error},
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The switch's element id, which the row's parts extend.
const ID: &str = "settings-daily-report";

type SaveAction = FutureAction<Option<ReportTime>, (), Message>;

pub struct DailyReportSection {
  /// The setting as shown: `None` until it is read, then the time the report comes at, or `None` while it is off.
  shown: Signal<Option<Option<ReportTime>>>,
  /// The time's select, `HH:MM`.
  time: Signal<String>,
}

impl Component for DailyReportSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      shown: ctx.signal(None),
      time: ctx.signal(clock(ReportTime::DEFAULT.minute())),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let failed = save.state().get().error;
    let read = stored(ctx);
    if self.shown.get_untracked().is_none()
      && let Some(stored) = read.data
    {
      self.shown.set(Some(stored));
      show(&self.time, clock(stored.unwrap_or(ReportTime::DEFAULT).minute()));
    }
    let shown = self.shown.get();
    self.follow_time(shown, &save);
    let title = ctx.t("settings.dailyReport.title");
    let description = ctx.t("settings.dailyReport.description");
    let on = matches!(shown, Some(Some(_)));
    let switch = Toggle::new(ID, ToggleKind::Switch, on, title.clone()).hide_label();
    let switch = match shown {
      Some(_) => self.listen(switch, save),
      None => switch,
    };
    let problem = match (&read.error, &failed) {
      (Some(reason), _) => Some(("settings.dailyReport.unreadable", reason)),
      (None, Some(reason)) => Some(("settings.dailyReport.notSaved", reason)),
      (None, None) => None,
    }
    .map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      Column::new().padding_bottom(SpacingSize::Sm).child(field_error(
        &format!("{ID}-error"),
        &ctx.t_args(key, [("reason", &*reason)]),
      ))
    });
    Column::new()
      .id(format!("{ID}-section"))
      .width(Dimension::Pct(100.0))
      .child(row(ID, (&title, &description), switch))
      .with_children(on.then(|| self.time_row(ctx)))
      .with_children(problem)
  }
}

impl DailyReportSection {
  /// `switch`, turning the report on at the time the select shows, or off, and saving that.
  fn listen(&self, switch: Toggle, save: SaveAction) -> Toggle {
    let (shown, time) = (self.shown.clone(), self.time.clone());
    switch.on_change(move |chosen| {
      let at = chosen.then(|| {
        minute(&time.get_untracked())
          .and_then(ReportTime::new)
          .unwrap_or(ReportTime::DEFAULT)
      });
      shown.set(Some(at));
      save.run(at);
    })
  }

  /// Takes the time the person chose in the select while the report is on.
  fn follow_time(&self, shown: Option<Option<ReportTime>>, save: &SaveAction) {
    let Some(Some(stored)) = shown else {
      return;
    };
    let Some(chosen) = minute(&self.time.get()).and_then(ReportTime::new) else {
      return;
    };
    if chosen != stored {
      self.shown.set(Some(Some(chosen)));
      save.run(Some(chosen));
    }
  }

  /// *Deliver at* and the time's select.
  fn time_row(&self, ctx: &Ctx) -> Element {
    let select = SelectField::new(
      format!("{ID}-time"),
      ctx.t("settings.dailyReport.time"),
      self.time.clone(),
      time_options(),
    )
    .width(Dimension::Px(TIME_WIDTH))
    .render(ctx);
    Row::new()
      .id(format!("{ID}-at"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(&ctx.t("settings.dailyReport.at")))
      .child(select)
      .padding_bottom(ExtraSpacing::Snug.role())
      .into()
  }
}

/// Saves the time the person chose, or that the report is off.
fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |time: Option<ReportTime>| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_daily_report(time))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "the daily report setting was not saved");
          messages::store_error(&error)
        })
    }
  })
}

/// The setting as stored, read when the section opens.
fn stored(ctx: &mut Ctx) -> FutureState<Option<ReportTime>, Message> {
  let workspace = Services::of(ctx).workspace;
  ctx
    .future((), move |()| {
      let workspace = workspace.clone();
      async move {
        background::run(move || workspace.daily_report_schedule())
          .await?
          .map(|schedule| schedule.map(|schedule| schedule.time))
          .map_err(|error| {
            tracing::error!(%error, "the daily report setting cannot be read");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
}
