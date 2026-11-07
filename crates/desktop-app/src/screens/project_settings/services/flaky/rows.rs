//! The page's groups (`Pane` `Bmqbo`, 28 apart): Quarantined (`j4jh7`) and Watching (`M8lqEE`), each its heading with
//! its count right after it over its rows, then Rule (`x1gaUf`). A test's row (`YF8Pd`, `j3xSrC`) is its name in Geist
//! Mono over its suite and when it was last seen, and at its end how often it was flaky in 12 `warning` over where it
//! stands in 11 `text_muted`, 1 apart and at their end (`D9awY5`), then *Unquarantine* (`Button/Secondary`, `undo-2`)
//! or *Quarantine now* (`Button/Ghost`, `shield`). Watching lists every flaky test not quarantined, watched ones and
//! those still under the rule. The rule's figures are fixed in Orchester, so its fields are locked (`thwDK`, `U3wqJx`).
//!
//! The history keeps a test's suite and name, not its file and line nor the task of the run it was seen in, so the
//! hint says the suite and when; the window counts the runs it was flaky in, not the runs it ran in.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  test_step::flaky::{FlakyTest, Standing},
  time::Timestamp,
};
use orchester_workspace::{display, messages::Message};

use super::{super::groups::count, DAY_MILLIS, Move, Moves, Read, window_days};
use crate::{
  screens::project_settings::kit::{Group, SettingRow, Tone},
  theme::{ExtraSpacing, ExtraTypography, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    message::message_text,
  },
};

/// The rule's run count field (`Input` `thwDK`).
const RUNS_FIELD_WIDTH: f32 = 120.0;
/// The rule's day count field (`Input` `D8tq0H`).
const DAYS_FIELD_WIDTH: f32 = 90.0;
/// What a quarantined test does (`Select` `U3wqJx`).
const OUTCOME_FIELD_WIDTH: f32 = 190.0;

/// What the rule's locked fields show.
#[derive(Clone)]
pub(super) struct RuleValues {
  pub(super) runs: Signal<String>,
  pub(super) days: Signal<String>,
  pub(super) outcome: Signal<String>,
}

/// Quarantined and Watching for the tests `listed`.
pub(super) fn groups(ctx: &mut Ctx, moves: &Moves, listed: &Read) -> Vec<Element> {
  let (quarantined, watching): (Vec<&FlakyTest>, Vec<&FlakyTest>) = listed
    .tests
    .iter()
    .partition(|test| matches!(test.standing, Standing::Quarantined { .. }));
  vec![
    quarantined_group(ctx, moves, &quarantined, listed.at),
    watching_group(ctx, moves, &watching, listed.at),
  ]
}

fn quarantined_group(ctx: &mut Ctx, moves: &Moves, tests: &[&FlakyTest], now: Timestamp) -> Element {
  const ID: &str = "settings-flaky-quarantined";
  let group = Group::new(ID, ctx.t("project_settings.services.flakyTests.quarantined")).beside(count(ID, tests.len()));
  if tests.is_empty() {
    return group
      .row(empty_row(ctx, &format!("{ID}-empty"), "quarantinedEmpty"))
      .into();
  }
  let rows: Vec<Element> = tests
    .iter()
    .enumerate()
    .map(|(index, test)| test_row(ctx, moves, (&format!("{ID}-{index}"), test), now))
    .collect();
  group.rows(rows).into()
}

fn watching_group(ctx: &mut Ctx, moves: &Moves, tests: &[&FlakyTest], now: Timestamp) -> Element {
  const ID: &str = "settings-flaky-watching";
  let note = Text::new(&ctx.t("project_settings.services.flakyTests.watchingNote"))
    .id(format!("{ID}-note"))
    .variant(ExtraTypography::Note.role())
    .nowrap();
  let group = Group::new(ID, ctx.t("project_settings.services.flakyTests.watching"))
    .beside(count(ID, tests.len()))
    .end(note);
  if tests.is_empty() {
    return group
      .row(empty_row(ctx, &format!("{ID}-empty"), "watchingEmpty"))
      .into();
  }
  let rows: Vec<Element> = tests
    .iter()
    .enumerate()
    .map(|(index, test)| test_row(ctx, moves, (&format!("{ID}-{index}"), test), now))
    .collect();
  group.rows(rows).into()
}

/// A group's row while it has no test: its title and what brings one, under `key`.
fn empty_row(ctx: &mut Ctx, id: &str, key: &str) -> Element {
  let words = |part: &str| ctx.t(&format!("project_settings.services.flakyTests.{key}.{part}"));
  let (title, detail) = (words("title"), words("detail"));
  SettingRow::new(id.to_owned(), title)
    .label_color(Tone::Secondary.color())
    .hint(detail)
    .render(ctx)
}

/// A test's row `id`: its name, suite and when it was last seen; how often it was flaky and where it stands; its
/// action, and under it why its move failed or was refused.
fn test_row(ctx: &mut Ctx, moves: &Moves, (id, test): (&str, &FlakyTest), now: Timestamp) -> Element {
  let key = &test.test;
  let state = moves.action.state().get();
  let moving = moves.moving(key);
  let quarantined = matches!(test.standing, Standing::Quarantined { .. });
  let button = if quarantined {
    ActionButton::new(
      format!("{id}-release"),
      ButtonKind::Secondary,
      ctx.t("project_settings.services.flakyTests.unquarantine"),
    )
    .icon(Icon::Undo2)
  } else {
    ActionButton::new(
      format!("{id}-quarantine"),
      ButtonKind::Ghost,
      ctx.t("project_settings.services.flakyTests.quarantineNow"),
    )
    .icon(Icon::Shield)
  };
  let button = if moving && state.is_pending() {
    button.loading()
  } else {
    let (moves, key) = (moves.clone(), key.clone());
    button.on_click(move || {
      moves.press(if quarantined {
        Move::Release(key.clone())
      } else {
        Move::Quarantine(key.clone())
      });
    })
  };
  let refused: Option<Element> = moves.refused(key).then(|| {
    field_error(
      &format!("{id}-person-only"),
      &ctx.t("project_settings.services.flakyTests.personOnly"),
    )
    .into()
  });
  let failure: Option<Element> = state
    .error
    .as_ref()
    .filter(|_| moving && state.is_rejected())
    .map(|error| field_error(&format!("{id}-error"), &message_text(ctx, error)).into());
  SettingRow::new(id.to_owned(), test.test.test().to_owned())
    .mono_label()
    .hint(hint(ctx, test, now))
    .control(standing(ctx, id, test))
    .control(button)
    .under_all(refused)
    .under_all(failure)
    .render(ctx)
}

/// The test's suite and when it was last seen flaky or failing in quarantine: earlier today how long ago,
/// then *yesterday*, then its date.
fn hint(ctx: &mut Ctx, test: &FlakyTest, now: Timestamp) -> Arc<str> {
  let suite = test.test.suite().as_str().to_owned();
  let Some(seen) = test.last_flaky.max(test.last_quarantined_failure) else {
    return suite.into();
  };
  let today = now.local_day_start();
  if seen >= today {
    let ago = message_text(ctx, &display::ago(now, seen)).to_string();
    return ctx.t_args(
      "project_settings.services.flakyTests.seenAgo",
      [("suite", suite), ("ago", ago)],
    );
  }
  if seen >= Timestamp::from_unix_millis(today.unix_millis() - DAY_MILLIS).local_day_start() {
    return ctx.t_args("project_settings.services.flakyTests.seenYesterday", [("suite", suite)]);
  }
  let (month, day) = date(ctx, seen);
  ctx.t_args(
    "project_settings.services.flakyTests.seenOn",
    [("suite", suite), ("month", month), ("day", day)],
  )
}

/// `at`'s month's short name and its day, as dates on the page read.
fn date(ctx: &mut Ctx, at: Timestamp) -> (String, String) {
  let (month, day) = display::month_day(at);
  let month = message_text(ctx, &Message::new(display::MONTH_KEYS[month])).to_string();
  (month, day.to_string())
}

/// How often the test was flaky in the window, in `warning` while it was, over where it stands (`Flaky` `HYqjz`).
fn standing(ctx: &mut Ctx, id: &str, test: &FlakyTest) -> Element {
  let (rate, tone) = match test.flaky_runs {
    0 => (
      ctx.t_args(
        "project_settings.services.flakyTests.rate.none",
        [("days", window_days().to_string())],
      ),
      Tone::Muted,
    ),
    1 => (ctx.t("project_settings.services.flakyTests.rate.one"), Tone::Warning),
    runs => (
      ctx.t_args(
        "project_settings.services.flakyTests.rate.other",
        [("count", runs.to_string())],
      ),
      Tone::Warning,
    ),
  };
  let stands = match test.standing {
    Standing::Quarantined { since } => {
      let (month, day) = date(ctx, since);
      ctx.t_args(
        "project_settings.services.flakyTests.since",
        [("month", month), ("day", day)],
      )
    }
    Standing::Watched => ctx.t("project_settings.services.flakyTests.watched"),
    Standing::Seen => ctx.t("project_settings.services.flakyTests.notQuarantined"),
  };
  Column::new()
    .id(format!("{id}-standing"))
    .align_items(Alignment::End)
    .spacing(ExtraSpacing::Hair.role())
    .child(
      Text::new(&rate)
        .id(format!("{id}-rate"))
        .variant(ProjectSettingsText::Hint.role())
        .color(tone.color())
        .nowrap(),
    )
    .child(
      Text::new(&stands)
        .id(format!("{id}-stands"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}

/// Rule (`x1gaUf`): after how many flaky runs in how many days a test is watched, and what a quarantined test does,
/// each in locked fields since Orchester fixes them.
pub(super) fn rule(ctx: &mut Ctx, values: &RuleValues) -> Element {
  const ID: &str = "settings-flaky-rule";
  let t = |ctx: &Ctx, key: &str| ctx.t(&format!("project_settings.services.flakyTests.rule.{key}"));
  let runs = FieldInput::new(format!("{ID}-runs"), t(ctx, "runsName"), values.runs.clone())
    .mono()
    .unit(t(ctx, "runs"))
    .width(Dimension::Px(RUNS_FIELD_WIDTH))
    .locked()
    .render(ctx);
  let days = FieldInput::new(format!("{ID}-days"), t(ctx, "daysName"), values.days.clone())
    .mono()
    .unit(t(ctx, "days"))
    .width(Dimension::Px(DAYS_FIELD_WIDTH))
    .locked()
    .render(ctx);
  let window = Row::new()
    .id(format!("{ID}-window"))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(runs)
    .child(Text::new(&t(ctx, "in")).variant(ProjectSettingsText::Hint.role()))
    .child(days);
  let after = SettingRow::new(format!("{ID}-after"), t(ctx, "after"))
    .hint(t(ctx, "afterHint"))
    .control(window)
    .render(ctx);
  let outcome_field = FieldInput::new(format!("{ID}-outcome-value"), t(ctx, "outcome"), values.outcome.clone())
    .width(Dimension::Px(OUTCOME_FIELD_WIDTH))
    .locked()
    .render(ctx);
  let outcome = SettingRow::new(format!("{ID}-outcome"), t(ctx, "outcome"))
    .hint(t(ctx, "outcomeHint"))
    .control(outcome_field)
    .render(ctx);
  Group::new(ID, t(ctx, "title")).row(after).row(outcome).into()
}
