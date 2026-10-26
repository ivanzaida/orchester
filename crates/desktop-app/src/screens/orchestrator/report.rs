//! The project's daily report in the Orchestrator's conversation (ORC-47, `orchester_core::orchestrator::report`;
//! `Report · closed` and `Report · open` in `4 · Report and the rest` `VZKyw`): one line in a `border` frame, radius
//! 10, 40 high, 12 at its sides, 10 apart: a 14 `newspaper`, *Daily report · day* (12.5/500), its totals (a figure in
//! Geist Mono 11.5 in its tone over its words in 12 `text_muted`: merged in `success`, waiting for the person in
//! `warning`, tokens), when it was delivered and the chevron that opens it. Open, its head is on `surface_raised` and
//! its sections show under it, 36 in: what the Orchestrator does with it while *Let the Orchestrator answer agents* is
//! on, and the report itself as the chat shows what an agent wrote (`ui::chat::agent_text`), each part a heading over
//! its list of labelled links (`Orchestrator · reply with links` `w262B`), so every item opens its place in the app.
//! The Orchestrator's summary is its answer, under it.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{
    DailyReport, RunEnd, StopReason,
    report::{ActionWay, MergedBy, PersonWaitKind, ReportRun},
  },
  run::RunLimit,
  time::Timestamp,
  tokens::compact_tokens,
  usage::local_date,
};
use orchester_workspace::display;

use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    chat::{agent_text, names::ProjectNames},
    icon::{Icon, icon},
    markdown::LinkHandler,
    message::message_text,
    pressable::pressable,
  },
};

/// A value a line has nothing for.
const NONE: &str = "—";
/// The head's height (`SEujT`).
const HEAD_HEIGHT: f32 = 40.0;

/// The report of a row, `id` its card: the report, when it was kept, whether it started a turn by itself; the
/// project, the names of the projects and what a press on a link does.
#[derive(Clone)]
pub(super) struct ReportProps {
  pub(super) id: String,
  pub(super) report: DailyReport,
  pub(super) at: Option<Timestamp>,
  pub(super) turned: bool,
  pub(super) project: ProjectId,
  pub(super) names: ProjectNames,
  pub(super) links: LinkHandler,
}

/// Compared by what it shows; the links' handler is the screen's.
impl PartialEq for ReportProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.report, self.at, self.turned, self.project, &self.names)
      == (&other.id, &other.report, other.at, other.turned, other.project, &other.names)
  }
}

impl DevtoolsInspectable for ReportProps {}

pub(super) struct ReportCard {
  open: Signal<bool>,
}

impl Component for ReportCard {
  type Props = ReportProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ReportProps>().clone();
    let id = props.id.as_str();
    let open = self.open.get();
    let sections = open.then(|| sections(ctx, &props));
    Column::new()
      .id(id.to_owned())
      .describe("outcome", "daily_report")
      .describe("day", day_label(&props.report))
      .describe("automatic", props.report.automatic.to_string())
      .describe("open", open.to_string())
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .clip()
      .child(head(ctx, &props, &self.open))
      .with_children(sections)
  }
}

/// The head: the day, its totals, when it was delivered and the chevron.
fn head(ctx: &Ctx, props: &ReportProps, open: &Signal<bool>) -> Element {
  let id = props.id.as_str();
  let report = &props.report;
  let shown = open.get();
  let title = ctx.t_args("orchestrator.report.title", [("day", super::day(ctx, report.day))]);
  let total = |suffix: &str, (figure, color): (String, PaletteColor), words: Arc<str>| {
    Row::new()
      .id(format!("{id}-{suffix}"))
      .spacing(SpacingSize::Xs)
      .align_items(Alignment::Center)
      .child(Text::new(&figure).variant(ExtraTypography::MonoKey.role()).color(color).nowrap())
      .child(
        Text::new(&words)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .nowrap(),
      )
  };
  let toggle = open.clone();
  let row = pressable(move || toggle.update(|open| *open = !*open))
    .id(format!("{id}-head"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEAD_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Newspaper, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(&title)
        .id(format!("{id}-title"))
        .variant(OrchestratorText::Emphasis.role())
        .nowrap(),
    )
    .child(total(
      "merged",
      (report.merged.len().to_string(), PaletteColor::Success),
      ctx.t("orchestrator.report.totals.merged"),
    ))
    .child(total(
      "waiting",
      (report.waiting.len().to_string(), PaletteColor::Warning),
      ctx.t("orchestrator.report.totals.waiting"),
    ))
    .child(total(
      "tokens",
      (compact_tokens(report.usage.spent), PaletteColor::TextMuted),
      ctx.t("orchestrator.report.totals.tokens"),
    ))
    .child(Spacer::new().flex(1.0))
    .with_children(props.at.map(|at| {
      Text::new(&delivered(ctx, report, at))
        .id(format!("{id}-state"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }))
    .child(icon(
      if shown { Icon::ChevronUp } else { Icon::ChevronDown },
      IconSize::Sm,
      PaletteColor::TextMuted,
    ));
  if shown {
    row.background(PaletteColor::SurfaceRaised).into()
  } else {
    row.into()
  }
}

/// The report's sections, 36 in under its head: what the Orchestrator does with it, then the report.
fn sections(ctx: &mut Ctx, props: &ReportProps) -> Element {
  let id = props.id.as_str();
  let effect = props.report.automatic.then(|| {
    let key = if props.turned {
      "orchestrator.report.effectSummary"
    } else {
      "orchestrator.report.effectQueued"
    };
    Text::new(&ctx.t(key))
      .id(format!("{id}-effect"))
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0))
  });
  let text: Arc<str> = Arc::from(Words { ctx }.markdown(&props.report).as_str());
  let body = agent_text(
    ctx,
    &format!("{id}-body"),
    &text,
    (props.project, &props.names),
    &props.links,
  );
  Column::new()
    .id(format!("{id}-sections"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xs)
        .right(SpacingSize::Md)
        .bottom(SpacingSize::Md)
        .left(OrchestratorSpacing::Report.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .with_children(effect)
    .child(body)
    .into()
}

/// When `report` was delivered at `at`: its time, and its day when that is a later day than the report's.
fn delivered(ctx: &Ctx, report: &DailyReport, at: Timestamp) -> Arc<str> {
  let time = display::clock(at);
  let on = local_date(at);
  if on == report.day {
    time.into()
  } else {
    ctx.t_args(
      "orchestrator.report.deliveredOn",
      [("day", super::day(ctx, on).to_string()), ("time", time)],
    )
  }
}

/// The report's day as tools read it (`2026-10-26`).
fn day_label(report: &DailyReport) -> String {
  report.day.format("%Y-%m-%d").to_string()
}

/// The report's words, as the chat's Markdown: each part's heading, then its lines.
struct Words<'a> {
  ctx: &'a Ctx,
}

impl Words<'_> {
  fn markdown(&self, report: &DailyReport) -> String {
    let parts = [
      (
        "orchestrator.report.section.done",
        self.done(report),
        "orchestrator.report.none.done",
      ),
      (
        "orchestrator.report.section.stands",
        self.stands(report),
        "orchestrator.report.none.stands",
      ),
      (
        "orchestrator.report.section.waits",
        self.waits(report),
        "orchestrator.report.none.waits",
      ),
      ("orchestrator.report.section.usage", self.usage(report), ""),
      (
        "orchestrator.report.section.next",
        self.next(report),
        "orchestrator.report.none.next",
      ),
    ];
    let mut text = String::new();
    for (heading, lines, none) in parts {
      text.push_str(&format!("**{}**\n\n", escape(&self.ctx.t(heading))));
      if lines.is_empty() {
        text.push_str(&format!("{}\n\n", escape(&self.ctx.t(none))));
      } else {
        text.push_str(&lines.join("\n"));
        text.push_str("\n\n");
      }
    }
    let built = display::clock(report.built);
    text.push_str(&escape(&self.ctx.t_args("orchestrator.report.asOf", [("time", built)])));
    text
  }

  fn line(&self, key: &str, args: &[(&str, String)]) -> String {
    self
      .ctx
      .t_args(key, args.iter().map(|(name, value)| (*name, value.clone())))
      .to_string()
  }

  /// What was merged and what the Orchestrator did, in the order it happened.
  fn done(&self, report: &DailyReport) -> Vec<String> {
    let merged = report.merged.iter().map(|task| {
      let key = match task.by {
        MergedBy::Person => "orchestrator.report.merged",
        MergedBy::Orchestrator => "orchestrator.report.mergedByOrchestrator",
        MergedBy::OrchestratorOnItsOwn => "orchestrator.report.mergedOnItsOwn",
      };
      self.line(
        key,
        &[
          ("task", escape(&task.task)),
          ("title", escape(&task.title)),
          ("link", destination(&task.link)),
          ("run", task.number.to_string()),
          ("time", display::clock(task.at)),
        ],
      )
    });
    let actions = report.actions.iter().map(|action| {
      let key = match action.way {
        ActionWay::Direct => "orchestrator.report.action.direct",
        ActionWay::Applied => "orchestrator.report.action.applied",
      };
      let what = action
        .task
        .as_deref()
        .or(action.subject.as_deref())
        .unwrap_or(&action.tool);
      self.line(
        key,
        &[
          ("what", escape(what)),
          ("link", destination(&action.link)),
          ("tool", escape(&action.tool)),
          ("time", display::clock(action.at)),
        ],
      )
    });
    merged.chain(actions).collect()
  }

  /// The runs ready to merge, stopped with what the person can do, and at work.
  fn stands(&self, report: &DailyReport) -> Vec<String> {
    let ready = report
      .ready
      .iter()
      .map(|run| self.run("orchestrator.report.ready", run));
    let stopped = report
      .stopped
      .iter()
      .map(|stopped| self.run(stopped_key(stopped.end), &stopped.run));
    let working = report
      .working
      .iter()
      .map(|run| self.run("orchestrator.report.working", run));
    ready.chain(stopped).chain(working).collect()
  }

  fn run(&self, key: &str, run: &ReportRun) -> String {
    self.line(
      key,
      &[
        ("task", escape(&run.task)),
        ("title", escape(&run.title)),
        ("link", destination(&run.link)),
        ("run", run.number.to_string()),
        ("step", run.step.as_deref().map_or_else(|| NONE.to_owned(), escape)),
      ],
    )
  }

  /// What waits for the person.
  fn waits(&self, report: &DailyReport) -> Vec<String> {
    report
      .waiting
      .iter()
      .map(|wait| {
        let key = match wait.kind {
          PersonWaitKind::Approval => "orchestrator.report.wait.approval",
          PersonWaitKind::Question => "orchestrator.report.wait.question",
          PersonWaitKind::Message => "orchestrator.report.wait.message",
          PersonWaitKind::Plan => "orchestrator.report.wait.plan",
          PersonWaitKind::Proposal => "orchestrator.report.wait.proposal",
        };
        self.line(
          key,
          &[
            ("task", wait.task.as_deref().map_or_else(|| NONE.to_owned(), escape)),
            (
              "run",
              wait.number.map_or_else(|| NONE.to_owned(), |number| number.to_string()),
            ),
            ("what", wait.what.as_deref().map_or_else(|| NONE.to_owned(), escape)),
            ("link", destination(&wait.link)),
          ],
        )
      })
      .collect()
  }

  /// The day's tokens and agent time against the daily budget, then by task and by account.
  fn usage(&self, report: &DailyReport) -> Vec<String> {
    let usage = &report.usage;
    let total = match usage.budget {
      Some(budget) => self.line(
        "orchestrator.report.usage.total",
        &[
          ("spent", compact_tokens(usage.spent)),
          ("budget", compact_tokens(budget)),
          ("link", destination(&usage.link)),
          ("time", self.active(usage.active_ms)),
        ],
      ),
      None => self.line(
        "orchestrator.report.usage.totalUnlimited",
        &[
          ("spent", compact_tokens(usage.spent)),
          ("link", destination(&usage.link)),
          ("time", self.active(usage.active_ms)),
        ],
      ),
    };
    let tasks = usage.tasks.iter().map(|share| {
      self.line(
        "orchestrator.report.usage.task",
        &[
          ("task", escape(&share.task)),
          ("title", escape(&share.title)),
          ("link", destination(&share.link)),
          ("spent", compact_tokens(share.spent)),
          ("time", self.active(share.active_ms)),
        ],
      )
    });
    let accounts = usage.accounts.iter().map(|share| {
      let name = match (&share.name, share.account) {
        (Some(name), _) => escape(name),
        (None, Some(account)) => account.to_string(),
        (None, None) => self.ctx.t("orchestrator.report.usage.noAccount").to_string(),
      };
      self.line(
        "orchestrator.report.usage.account",
        &[
          ("name", name),
          ("link", destination(&share.link)),
          ("spent", compact_tokens(share.spent)),
          ("time", self.active(share.active_ms)),
        ],
      )
    });
    std::iter::once(total).chain(tasks).chain(accounts).collect()
  }

  fn active(&self, ms: u64) -> String {
    message_text(self.ctx, &display::duration(Duration::from_millis(ms))).to_string()
  }

  /// The queued tasks, in the order they are queued.
  fn next(&self, report: &DailyReport) -> Vec<String> {
    report
      .queued
      .iter()
      .enumerate()
      .map(|(index, task)| {
        self.line(
          "orchestrator.report.queued",
          &[
            ("place", (index + 1).to_string()),
            ("task", escape(&task.task)),
            ("title", escape(&task.title)),
            ("link", destination(&task.link)),
          ],
        )
      })
      .collect()
  }
}

/// The line of a run that stopped so, which says what the person can do.
const fn stopped_key(end: RunEnd) -> &'static str {
  match end {
    RunEnd::Succeeded => "orchestrator.report.ready",
    RunEnd::Failed => "orchestrator.report.stopped.failed",
    RunEnd::Stopped { why } => match why {
      StopReason::RetriesExhausted { .. } => "orchestrator.report.stopped.retries",
      StopReason::BrokeAgain => "orchestrator.report.stopped.brokeAgain",
      StopReason::NoRoute => "orchestrator.report.stopped.noRoute",
      StopReason::Unverified => "orchestrator.report.stopped.unverified",
      StopReason::Unchanged => "orchestrator.report.stopped.unchanged",
    },
    RunEnd::Limit { limit } => match limit {
      RunLimit::Tokens => "orchestrator.report.stopped.limitTokens",
      RunLimit::Time => "orchestrator.report.stopped.limitTime",
      RunLimit::DailyBudget => "orchestrator.report.stopped.limitDaily",
      RunLimit::GroupBudget => "orchestrator.report.stopped.limitGroup",
      RunLimit::StepBudget { .. } => "orchestrator.report.stopped.limitStep",
      RunLimit::PlanLimit { .. } => "orchestrator.report.stopped.limitPlan",
    },
    RunEnd::Interrupted => "orchestrator.report.stopped.interrupted",
  }
}

/// An app link as a Markdown destination the chat opens in the app.
fn destination(link: &str) -> String {
  format!("orchester:{link}")
}

/// `text` with the characters Markdown reads as markup escaped, so a title shows as written.
fn escape(text: &str) -> String {
  let mut escaped = String::with_capacity(text.len());
  for character in text.chars() {
    if matches!(
      character,
      '\\' | '`' | '*' | '_' | '[' | ']' | '(' | ')' | '<' | '>' | '#' | '|' | '!'
    ) {
      escaped.push('\\');
    }
    escaped.push(character);
  }
  escaped
}
