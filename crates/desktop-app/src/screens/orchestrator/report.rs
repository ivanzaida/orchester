//! The project's daily report in the Orchestrator's conversation (ORC-47, `orchester_core::orchestrator::report`). No
//! design frame draws it yet: it takes the action card's frame (`Action card` `nQIhx`) as a wait and a run's end do
//! ([`super::wait`], [`super::run_end`]), its head with the day and, at its end, when it was delivered; what the
//! Orchestrator does with it while *Let the Orchestrator answer agents* is on (it adds a summary and changes nothing);
//! and the report itself as the chat shows what an agent wrote (`ui::chat::agent_text`), each part a heading over its
//! list of labelled links (`Orchestrator · reply with links` `w262B`), so every item opens its place in the app.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, TypographyStyle},
  },
  components::Text,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
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

use super::{
  card::{self, State},
  timeline::TurnEnv,
};
use crate::ui::{chat::agent_text, icon::Icon, message::message_text};

/// A value a line has nothing for.
const NONE: &str = "—";

/// The card of `report`, kept at `at`, which started an Orchestrator turn by itself when `turned`; `env` is the
/// conversation's; `id` names it.
pub(super) fn card(
  ctx: &mut Ctx,
  id: &str,
  (report, at, turned): (&DailyReport, Option<Timestamp>, bool),
  env: &TurnEnv,
) -> Element {
  let day = day_label(report);
  let title = ctx.t_args("orchestrator.report.title", [("day", day.clone())]);
  let state = State {
    glyph: Icon::CalendarClock,
    color: PaletteColor::TextSecondary,
    words: delivered(ctx, report, at),
  };
  let effect = report.automatic.then(|| {
    let key = if turned {
      "orchestrator.report.effectSummary"
    } else {
      "orchestrator.report.effectQueued"
    };
    Text::new(&ctx.t(key))
      .id(format!("{id}-effect"))
      .variant(TypographyStyle::Body)
      .width(Dimension::Pct(100.0))
  });
  let text: Arc<str> = Arc::from(Words { ctx }.markdown(report).as_str());
  let body = agent_text(ctx, &format!("{id}-body"), &text, (env.project, &env.names), &env.links);
  card::frame(id, "daily_report")
    .describe("day", day)
    .describe("automatic", report.automatic.to_string())
    .child(card::head(id, (Icon::CalendarDays, &title), None, state))
    .with_children(effect)
    .child(body)
    .into()
}

/// When `report` was delivered at `at`: its time, and its day when that is a later day than the report's.
fn delivered(ctx: &Ctx, report: &DailyReport, at: Option<Timestamp>) -> Arc<str> {
  let Some(at) = at else {
    return Arc::from("");
  };
  let time = display::clock(at);
  let on = local_date(at);
  if on == report.day {
    ctx.t_args("orchestrator.report.delivered", [("time", time)])
  } else {
    ctx.t_args(
      "orchestrator.report.deliveredOn",
      [("day", on.format("%Y-%m-%d").to_string()), ("time", time)],
    )
  }
}

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
