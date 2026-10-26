//! Settings › Notifications (`02 · Notifications` `fQTFi` in the new `orchester.pen`, ORC-56): *Notify me when* with a
//! switch per group of OS notifications (*Needs you* and *Stopped* on, *Finished and info* off until changed), *How*
//! with *Sound* and *Quiet hours*, which, while on, has its start and end on a row of their own, and the Orchestrator's
//! daily report (`super::daily_report`). A change shows at once, in the window's [`NotificationPrefs`], and is saved off
//! the UI thread; one that was not saved says why under the groups. A build that shows no OS notifications says so over
//! them.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::notification::{MINUTES_PER_DAY, NotificationGroup, NotificationSettings, QuietHours};
use orchester_workspace::{
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use super::{
  frame,
  save_state::{Outcome, SaveState},
};
use crate::{
  notifications::{
    NotificationPrefs,
    os::{Delivery, VARIABLE},
  },
  services::Services,
  theme::ExtraSpacing,
  ui::{
    field::{SelectField, field_error},
    message::message_text,
    notice::NoticeTone,
    setting_notice::SettingNotice,
    setting_rows::{SettingGroup, SettingRow},
    toggle::{Toggle, ToggleKind},
  },
};

/// The minutes between the times a time's select offers.
const STEP: u16 = 30;

type SaveAction = FutureAction<NotificationSettings, (), Message>;

/// The tab, scrolling under the header.
pub fn tab(ctx: &mut Ctx) -> Element {
  let pane = frame::pane("settings-notifications-tab")
    .child(ctx.mount_keyed::<NotificationsSection>("notifications", ()))
    .child(ctx.mount_keyed::<super::daily_report::DailyReportSection>("daily-report", ()));
  frame::column("settings-notifications-scroll", pane).into()
}

/// *Notify me when* and *How*.
pub struct NotificationsSection {
  /// The quiet hours' start and end as chosen, `HH:MM`.
  start: Signal<String>,
  end: Signal<String>,
  /// The quiet hours' start and end the selects were last set to or took; `None` while quiet hours are off.
  seen: Mutex<Option<(u16, u16)>>,
}

impl Component for NotificationsSection {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let quiet = NotificationPrefs::of(ctx).get().quiet.unwrap_or_default();
    Self {
      start: ctx.signal(clock(quiet.start())),
      end: ctx.signal(clock(quiet.end())),
      seen: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let prefs = NotificationPrefs::of(ctx);
    let settings = prefs.get();
    let save = save_action(ctx);
    let state = save.state().get();
    if let Some(saves) = SaveState::of(ctx) {
      saves.report("settings-notifications", Outcome::of(&state));
    }
    let failed = state.error;
    self.follow_times(&prefs, &save, settings);
    let mut switch = |key, change| switch_row(ctx, (&prefs, &save), key, change);
    let when = [
      switch(
        ("needsYou", settings.shows(NotificationGroup::NeedsYou), true),
        |settings, on| settings.with(NotificationGroup::NeedsYou, on),
      ),
      switch(
        ("stopped", settings.shows(NotificationGroup::Stopped), true),
        |settings, on| settings.with(NotificationGroup::Stopped, on),
      ),
      switch(
        ("finished", settings.shows(NotificationGroup::Finished), true),
        |settings, on| settings.with(NotificationGroup::Finished, on),
      ),
    ];
    // Sound says all it does in its name: no hint (`Sound` `JTj9y`).
    let mut how = vec![
      switch(("sound", settings.sound, false), |settings, on| NotificationSettings {
        sound: on,
        ..settings
      }),
      switch(("quiet", settings.quiet.is_some(), true), |settings, on| {
        NotificationSettings {
          quiet: on.then(QuietHours::default),
          ..settings
        }
      }),
    ];
    if settings.quiet.is_some() {
      how.push(self.times(ctx));
    }
    Column::new()
      .id("settings-notifications")
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::SettingsSpacing::Page.role())
      .with_children(delivery_note(ctx))
      .child(SettingGroup::new("settings-notify-when", ctx.t("settings.notifications.when")).rows(when))
      .child(SettingGroup::new("settings-notify-how", ctx.t("settings.notifications.how")).rows(how))
      .with_children(failed.map(|reason| {
        let reason = message_text(ctx, &reason);
        field_error(
          "settings-notify-error",
          &ctx.t_args("settings.notifications.notSaved", [("reason", &*reason)]),
        )
      }))
  }
}

impl NotificationsSection {
  /// Takes the quiet hours' times the person chose, once they are two different times. Quiet hours that come from
  /// elsewhere (the stored settings read after the section opened, or turned on with their default times) are only
  /// shown in the selects, and nothing is saved for them.
  fn follow_times(&self, prefs: &NotificationPrefs, save: &SaveAction, settings: NotificationSettings) {
    let chosen = (minute(&self.start.get()), minute(&self.end.get()));
    let mut seen = self.seen.lock();
    let Some(quiet) = settings.quiet else {
      *seen = None;
      return;
    };
    let stored = (quiet.start(), quiet.end());
    if *seen != Some(stored) {
      *seen = Some(stored);
      show(&self.start, clock(stored.0));
      show(&self.end, clock(stored.1));
      return;
    }
    let (Some(start), Some(end)) = chosen else {
      return;
    };
    if (start, end) == stored {
      return;
    }
    let Ok(hours) = QuietHours::new(start, end) else {
      return;
    };
    *seen = Some((start, end));
    let next = NotificationSettings {
      quiet: Some(hours),
      ..settings
    };
    prefs.set(next);
    save.run(next);
  }

  /// *From* with the quiet hours' start and end (`From 22:00 to 08:00`, `From` `HFMmS`); two same times say they are
  /// not taken.
  fn times(&self, ctx: &mut Ctx) -> Element {
    let select = |id: &str, name: &str, value: &Signal<String>| {
      SelectField::new(id, ctx.t(name), value.clone(), time_options())
        .width(Dimension::Px(TIME_WIDTH))
        .render(ctx)
    };
    let times = Row::new()
      .id("settings-notify-quiet-times")
      .align_items(Alignment::Center)
      .spacing(ExtraSpacing::Snug.role())
      .child(select(
        "settings-notify-quiet-start",
        "settings.notifications.quiet.start",
        &self.start,
      ))
      .child(
        Text::new(&ctx.t("settings.notifications.quiet.to"))
          .variant(lurq::app::theme::TypographyStyle::Description)
          .color(lurq::app::theme::PaletteColor::TextMuted),
      )
      .child(select(
        "settings-notify-quiet-end",
        "settings.notifications.quiet.end",
        &self.end,
      ));
    let same = (self.start.get() == self.end.get()).then(|| {
      field_error(
        "settings-notify-quiet-same",
        &ctx.t("settings.notifications.quiet.same"),
      )
      .into()
    });
    SettingRow::new("settings-notify-quiet-from", ctx.t("settings.notifications.quiet.from"))
      .control(times)
      .below_all(same)
      .render(ctx)
  }
}

/// The width of a time's select (`22:00` and its chevron).
pub(super) const TIME_WIDTH: f32 = 96.0;

/// The row `settings-notify-{key}` with its switch, titled by `settings.notifications.{key}.title` and, when `hinted`,
/// with the hint `….hint` under it, what it covers in its tooltip; turning the switch shows the settings `change`
/// makes at once and saves them.
fn switch_row(
  ctx: &mut Ctx,
  (prefs, save): (&NotificationPrefs, &SaveAction),
  (key, on, hinted): (&str, bool, bool),
  change: fn(NotificationSettings, bool) -> NotificationSettings,
) -> Element {
  let id = format!("settings-notify-{key}");
  let title = ctx.t(&format!("settings.notifications.{key}.title"));
  let description = ctx.t(&format!("settings.notifications.{key}.description"));
  let (prefs, save) = (prefs.clone(), save.clone());
  let toggle = Toggle::new(id.as_str(), ToggleKind::Switch, on, title.clone())
    .hide_label()
    .on_change(move |chosen| {
      let next = change(prefs.get_now(), chosen);
      prefs.set(next);
      save.run(next);
    });
  let row = SettingRow::new(id, title).info(description).control(toggle);
  let row = if hinted {
    row.hint(ctx.t(&format!("settings.notifications.{key}.hint")))
  } else {
    row
  };
  row.render(ctx)
}

/// What a build that shows no OS notifications says over the groups.
fn delivery_note(ctx: &Ctx) -> Option<Element> {
  (Delivery::of_build() == Delivery::Silent).then(|| {
    SettingNotice::new("settings-notify-silent", NoticeTone::Info)
      .body(ctx.t_args("settings.notifications.silent", [("variable", VARIABLE)]))
      .into()
  })
}

/// Saves the settings the person chose.
fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |settings: NotificationSettings| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.set_notification_settings(settings))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "the notification settings were not saved");
          messages::store_error(&error)
        })
    }
  })
}

/// `minute` of the day as `HH:MM`.
pub(super) fn clock(minute: u16) -> String {
  format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// The times of day a time's select offers, [`STEP`] apart: the quiet hours' ends and the daily report's time.
pub(super) fn time_options() -> Vec<SelectOption<String>> {
  (0..MINUTES_PER_DAY)
    .step_by(usize::from(STEP))
    .map(|minute| SelectOption::new(clock(minute), clock(minute)))
    .collect()
}

/// Sets a select's `value` to `time`, unless it shows it already.
pub(super) fn show(value: &Signal<String>, time: String) {
  if value.get_untracked() != time {
    value.set(time);
  }
}

/// The minute of the day `HH:MM` names.
pub(super) fn minute(clock: &str) -> Option<u16> {
  let (hours, minutes) = clock.split_once(':')?;
  let minute = hours.parse::<u16>().ok()? * 60 + minutes.parse::<u16>().ok()?;
  (minute < MINUTES_PER_DAY).then_some(minute)
}
