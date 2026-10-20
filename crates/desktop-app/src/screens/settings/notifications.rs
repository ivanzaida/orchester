//! *Notifications* (ORC-56) under *What automation may do*: a row with a switch per group of OS notifications (*Needs
//! you* and *Stopped* on, *Finished and info* off until changed), *Sound*, and *Quiet hours*, which, while on, has its
//! start and end on a row of their own. No design frame draws the section yet: it takes the Settings page's switch rows
//! (`Form row` `XaNIz`). A change shows at once, in the window's [`NotificationPrefs`], and is saved off the UI thread;
//! one that was not saved says why under the section. A build that shows no OS notifications says so over the rows.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::notification::{MINUTES_PER_DAY, NotificationGroup, NotificationSettings, QuietHours};
use orchester_workspace::{
  background,
  messages::{self, Message},
};
use parking_lot::Mutex;

use super::automation::{row, section_title};
use crate::{
  notifications::{
    NotificationPrefs,
    os::{Delivery, VARIABLE},
  },
  services::Services,
  theme::ExtraSpacing,
  ui::{
    field::{SelectField, field_error},
    icon::Icon,
    inspector::note,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// The minutes between the times quiet hours can start and end at.
const STEP: u16 = 30;

type SaveAction = FutureAction<NotificationSettings, (), Message>;

/// The section's rows.
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
    let failed = save.state().get().error;
    self.follow_times(&prefs, &save, settings);
    let switch = |ctx: &Ctx, (key, on): (&str, bool), change| switch_row(ctx, (&prefs, &save), (key, on), change);
    let mut rows: Vec<Element> = vec![
      switch(
        ctx,
        ("needsYou", settings.shows(NotificationGroup::NeedsYou)),
        |settings, on| settings.with(NotificationGroup::NeedsYou, on),
      ),
      switch(
        ctx,
        ("stopped", settings.shows(NotificationGroup::Stopped)),
        |settings, on| settings.with(NotificationGroup::Stopped, on),
      ),
      switch(
        ctx,
        ("finished", settings.shows(NotificationGroup::Finished)),
        |settings, on| settings.with(NotificationGroup::Finished, on),
      ),
      switch(ctx, ("sound", settings.sound), |settings, on| NotificationSettings {
        sound: on,
        ..settings
      }),
      switch(ctx, ("quiet", settings.quiet.is_some()), |settings, on| {
        NotificationSettings {
          quiet: on.then(QuietHours::default),
          ..settings
        }
      }),
    ];
    if settings.quiet.is_some() {
      rows.push(self.times(ctx));
    }
    Column::new()
      .id("settings-notifications")
      .width(Dimension::Pct(100.0))
      .child(section_title(&ctx.t("settings.notifications.section")))
      .with_children(delivery_note(ctx))
      .with_children(rows)
      .with_children(failed.map(|reason| {
        let reason = message_text(ctx, &reason);
        Column::new().padding_top(SpacingSize::Sm).child(field_error(
          "settings-notify-error",
          &ctx.t_args("settings.notifications.notSaved", [("reason", &*reason)]),
        ))
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

  /// The quiet hours' start and end (`From 22:00 to 08:00`); two same times say they are not taken.
  fn times(&self, ctx: &Ctx) -> Element {
    let options = || -> Vec<SelectOption<String>> {
      (0..MINUTES_PER_DAY)
        .step_by(usize::from(STEP))
        .map(|minute| SelectOption::new(clock(minute), clock(minute)))
        .collect()
    };
    let select = |id: &str, name: &str, value: &Signal<String>| {
      SelectField::new(id, ctx.t(name), value.clone(), options())
        .width(Dimension::Px(TIME_WIDTH))
        .render(ctx)
    };
    let same = self.start.get() == self.end.get();
    Column::new()
      .id("settings-notify-quiet-times")
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Md))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Row::new()
          .align_items(Alignment::Center)
          .spacing(SpacingSize::Sm)
          .child(Text::new(&ctx.t("settings.notifications.quiet.from")))
          .child(select(
            "settings-notify-quiet-start",
            "settings.notifications.quiet.start",
            &self.start,
          ))
          .child(Text::new(&ctx.t("settings.notifications.quiet.to")))
          .child(select(
            "settings-notify-quiet-end",
            "settings.notifications.quiet.end",
            &self.end,
          )),
      )
      .with_children(same.then(|| {
        field_error(
          "settings-notify-quiet-same",
          &ctx.t("settings.notifications.quiet.same"),
        )
      }))
      .into()
  }
}

/// The width of a time's select (`22:00` and its chevron).
const TIME_WIDTH: f32 = 112.0;

/// The row `settings-notify-{key}` with its switch, titled and described by `settings.notifications.{key}.*`; turning
/// the switch shows the settings `change` makes at once and saves them.
fn switch_row(
  ctx: &Ctx,
  (prefs, save): (&NotificationPrefs, &SaveAction),
  (key, on): (&str, bool),
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
  row(&id, (&title, &description), toggle).into()
}

/// What a build that shows no OS notifications says over the rows.
fn delivery_note(ctx: &Ctx) -> Option<Row> {
  (Delivery::of_build() == Delivery::Silent).then(|| {
    note(
      "settings-notify-silent",
      Icon::Info,
      &ctx.t_args("settings.notifications.silent", [("variable", VARIABLE)]),
    )
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
fn clock(minute: u16) -> String {
  format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// Sets a select's `value` to `time`, unless it shows it already.
fn show(value: &Signal<String>, time: String) {
  if value.get_untracked() != time {
    value.set(time);
  }
}

/// The minute of the day `HH:MM` names.
fn minute(clock: &str) -> Option<u16> {
  let (hours, minutes) = clock.split_once(':')?;
  let minute = hours.parse::<u16>().ok()? * 60 + minutes.parse::<u16>().ok()?;
  (minute < MINUTES_PER_DAY).then_some(minute)
}
