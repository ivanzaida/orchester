//! *At work* (`At work` `Xbz55` in 01 `LXhun`): one line per run under way (`orchester_workspace::overview::at_work`),
//! its head with how many and *N of M agent slots* as the runner reports them, and beside that, while ready steps wait
//! for a slot, how many and what would start them (no frame draws it; it reads like the slots). A line: the turning loader, the key,
//! the title in 13/500, the step's dots and the step (`GI561`: one 10×4 dash per step, 3 apart, the steps done in
//! `text_muted`, the current one in `text_primary`, the rest in `border`), the account and model with the maker's mark
//! (`VJWOi`), and how long the run has run. A run that lost its runner keeps its line and its age, with a pause mark.
//! Up to six lines; more opens Tasks on Active. A line opens its task's conversation, as the sidebar's Running does.

use std::num::NonZeroU32;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, project::TaskKey, time::Timestamp};
use orchester_workspace::overview::{AT_WORK_SHOWN, at_work::AtWorkLine};

use super::section::{LINE_HEIGHT, age, head, key, line, mono_time, muted, section, show_all, title};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskListTab, TaskView},
  services::accounts::{Accounts, account_name},
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    agent_mark::AgentMark,
    icon::{Icon, icon},
    pressable::{Press, opening},
  },
};

/// The step's room (`GI561`), the account's (`VJWOi`) and the age's (`QMEnN`); a step's dash (`C4rq2O`).
const STEP_WIDTH: f32 = 150.0;
const ACCOUNT_WIDTH: f32 = 196.0;
const AGE_WIDTH: f32 = 30.0;
const DASH: (f32, f32) = (10.0, 4.0);

/// The section; `slots` is what the runner reported in use of its agent slots, when it did, and `slot_waits` how many
/// ready steps it says wait for one.
pub(super) fn at_work(
  ctx: &mut Ctx,
  project: ProjectId,
  (lines, accounts): (&[AtWorkLine], &Accounts),
  (slots, slot_waits, now): (Option<(usize, NonZeroU32)>, usize, Timestamp),
) -> Column {
  let end = slots.map(|(working, slots)| {
    let key = if slots.get() == 1 {
      "overview.atWork.slots.one"
    } else {
      "overview.atWork.slots.other"
    };
    let words = ctx.t_args(key, [("count", working.to_string()), ("slots", slots.to_string())]);
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(muted(&words).id("overview-at-work-slots"))
      .with_children(waiting_steps(ctx, slot_waits))
      .into()
  });
  let navigation = Navigation::of(ctx);
  let shown: Vec<Element> = lines
    .iter()
    .take(AT_WORK_SHOWN)
    .map(|at_work| run_line(ctx, (&navigation, project), (at_work, accounts), now))
    .collect();
  let more = (lines.len() > AT_WORK_SHOWN).then(|| {
    let label = ctx.t_args("overview.atWork.showAll", [("count", lines.len().to_string())]);
    let open = navigation.to(Route::Project(
      project,
      ProjectPage::Tasks(TaskList::new(TaskListTab::Active)),
    ));
    show_all("overview-at-work-all", &label, Press::Open(open))
  });
  section("overview-at-work")
    .child(head(
      "overview-at-work",
      &ctx.t("overview.atWork.title"),
      Some(lines.len().to_string()),
      end,
    ))
    .with_children(shown)
    .with_children(more)
}

/// How many ready steps wait for an agent slot and what would start them, beside the slots while any do.
fn waiting_steps(ctx: &Ctx, count: usize) -> Option<Text> {
  let key = match count {
    0 => return None,
    1 => "overview.atWork.slotWaits.one",
    _ => "overview.atWork.slotWaits.other",
  };
  let words = ctx.t_args(key, [("count", count.to_string())]);
  Some(muted(&words).id("overview-at-work-slot-waits"))
}

fn run_line(
  ctx: &mut Ctx,
  (navigation, project): (&Navigation, ProjectId),
  (at_work, accounts): (&AtWorkLine, &Accounts),
  now: Timestamp,
) -> Element {
  let id = format!("overview-at-work-{}", at_work.key);
  let task: TaskKey = at_work.key.parse().expect("a task's key reads back as a task key");
  let (glyph, color) = if at_work.lost {
    (Icon::CirclePause, PaletteColor::Warning)
  } else {
    (Icon::LoaderCircle, PaletteColor::TextSecondary)
  };
  // A run that lost its runner keeps its age, as the runner-away state draws it (05 `JzNRO`).
  let age = at_work
    .work
    .as_ref()
    .map_or_else(|| "".into(), |work| age(ctx, work.started_at, now));
  let row = line(&id, LINE_HEIGHT, SpacingSize::Md)
    .describe("lost", at_work.lost.to_string())
    .child(icon(glyph, IconSize::Md, color))
    .child(key(&at_work.key))
    .child(title(
      &format!("{id}-title"),
      &at_work.title,
      ExtraTypography::Strong.role(),
      PaletteColor::TextPrimary,
    ))
    .child(step(ctx, &id, at_work))
    .child(agent(ctx, &id, at_work, accounts))
    .child(
      Row::new()
        .width(Dimension::Px(AGE_WIDTH))
        .flex_shrink(0.0)
        .justify(Justify::End)
        .child(mono_time(&format!("{id}-age"), &age)),
    );
  let route = Route::Project(project, ProjectPage::Task(task, TaskView::Conversation));
  opening(navigation.to(route))
    .id(format!("{id}-open"))
    .width(Dimension::Pct(100.0))
    .child(row)
    .into()
}

/// The step's dashes and its number and name.
fn step(ctx: &Ctx, id: &str, at_work: &AtWorkLine) -> Row {
  let steps = at_work.work.as_ref().map_or(0, |work| work.steps);
  let current = at_work.step.as_ref().map_or(0, |step| step.number);
  let dashes = (1..=steps).map(|number| {
    let fill = match number.cmp(&current) {
      std::cmp::Ordering::Less => PaletteColor::TextMuted,
      std::cmp::Ordering::Equal => PaletteColor::TextPrimary,
      std::cmp::Ordering::Greater => PaletteColor::Border,
    };
    Column::new()
      .width(Dimension::Px(DASH.0))
      .height(Dimension::Px(DASH.1))
      .rounded(ExtraRadius::Meter.role())
      .background(fill)
  });
  let name = at_work.step.as_ref().map(|step| {
    ctx.t_args(
      "overview.atWork.step",
      [
        ("number", format!("{:02}", step.number)),
        ("step", step.name.to_string()),
      ],
    )
  });
  Row::new()
    .id(format!("{id}-step"))
    .describe("steps", steps.to_string())
    .describe("current", current.to_string())
    .width(Dimension::Px(STEP_WIDTH))
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Fine.role())
        .align_items(Alignment::Center)
        .with_children(dashes),
    )
    // A long pipeline leaves the name less room than the cell: it ends in an ellipsis instead of being cut.
    .with_children(name.map(|name| {
      Text::new(&name)
        .id(format!("{id}-step-name"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0))
    }))
}

/// The account and model the step's agent runs on, after its maker's mark; nothing for a tool step.
fn agent(ctx: &Ctx, id: &str, at_work: &AtWorkLine, accounts: &Accounts) -> Row {
  let cell = Row::new()
    .id(format!("{id}-agent"))
    .width(Dimension::Px(ACCOUNT_WIDTH))
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  let Some(agent) = at_work.work.as_ref().and_then(|work| work.agent.as_ref()) else {
    return cell;
  };
  // An agent without an account (an API key, a native agent) names its model only: no account is made up.
  let words = match agent.account.and_then(|account| accounts.of(account)) {
    Some(account) => ctx.t_args(
      "overview.atWork.agent",
      [
        ("account", account_name(ctx, account)),
        ("model", agent.model.to_string()),
      ],
    ),
    None => agent.model.clone(),
  };
  cell
    .describe("model", agent.model.to_string())
    .with_children(
      agent
        .marked
        .then(|| AgentMark::of(agent.provider).render(ctx, IconSize::Sm)),
    )
    .child(
      muted(&words)
        .id(format!("{id}-agent-words"))
        .text_overflow(TextOverflow::Elipsis)
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
