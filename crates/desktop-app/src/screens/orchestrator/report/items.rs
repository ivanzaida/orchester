//! What an open daily report lists (`Report · open` in `VZKyw`): each part's heading and its items, every item a 12
//! icon in its tone, what it is and, at its end, what it adds; and the link into the app it opens. A part with no
//! items says so instead.

use std::{sync::Arc, time::Duration};

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  orchestrator::{
    DailyReport, RunEnd, StopReason,
    report::{ActionWay, MergedBy, PersonWaitKind, ReportRun, SharedBy},
  },
  run::RunLimit,
  tokens::compact_tokens,
};
use orchester_workspace::display;

use crate::ui::{icon::Icon, message::message_text};

/// A value a line has nothing for.
const NONE: &str = "—";

/// An item of a part: its icon and tone, what it is, what it adds at its end, what its tooltip says when it says
/// more, and the app link it opens.
pub(super) struct Item {
  pub(super) glyph: Icon,
  pub(super) tone: PaletteColor,
  pub(super) text: Arc<str>,
  pub(super) meta: Arc<str>,
  pub(super) tip: Option<Arc<str>>,
  pub(super) link: String,
}

/// A part of the report: its heading, its items and what it says without any (`None`: it always has some).
pub(super) struct Part {
  pub(super) heading: Arc<str>,
  pub(super) items: Vec<Item>,
  pub(super) none: Option<Arc<str>>,
}

/// The parts of `report`, in the order the report reads.
pub(super) fn parts(ctx: &Ctx, report: &DailyReport) -> Vec<Part> {
  let words = Words { ctx };
  let part = |heading: &str, items: Vec<Item>, none: Option<&str>| Part {
    heading: ctx.t(heading),
    items,
    none: none.map(|none| ctx.t(none)),
  };
  vec![
    part(
      "orchestrator.report.section.done",
      words.done(report),
      Some("orchestrator.report.none.done"),
    ),
    part(
      "orchestrator.report.section.stands",
      words.stands(report),
      Some("orchestrator.report.none.stands"),
    ),
    part(
      "orchestrator.report.section.waits",
      words.waits(report),
      Some("orchestrator.report.none.waits"),
    ),
    part("orchestrator.report.section.usage", words.usage(report), None),
    part(
      "orchestrator.report.section.next",
      words.next(report),
      Some("orchestrator.report.none.next"),
    ),
  ]
}

struct Words<'a> {
  ctx: &'a Ctx,
}

impl Words<'_> {
  fn t(&self, key: &str, args: &[(&str, String)]) -> Arc<str> {
    self
      .ctx
      .t_args(key, args.iter().map(|(name, value)| (*name, value.clone())))
  }

  fn task(&self, task: &str, title: &str) -> Arc<str> {
    self.t(
      "orchestrator.report.item.task",
      &[("task", task.to_owned()), ("title", title.to_owned())],
    )
  }

  /// What was merged and what the Orchestrator did, in the order it happened.
  fn done(&self, report: &DailyReport) -> Vec<Item> {
    let merged = report.merged.iter().map(|task| {
      let key = match task.by {
        MergedBy::Person => "orchestrator.report.item.merged",
        MergedBy::Orchestrator => "orchestrator.report.item.mergedByOrchestrator",
        MergedBy::OrchestratorOnItsOwn => "orchestrator.report.item.mergedOnItsOwn",
      };
      Item {
        glyph: Icon::GitMerge,
        tone: PaletteColor::Success,
        text: self.task(&task.task, &task.title),
        meta: self.t(
          key,
          &[("run", task.number.to_string()), ("time", display::clock(task.at))],
        ),
        tip: None,
        link: task.link.clone(),
      }
    });
    let actions = report.actions.iter().map(|action| {
      let (key, glyph, tone) = match action.way {
        ActionWay::Direct => (
          "orchestrator.report.item.direct",
          Icon::Waypoints,
          PaletteColor::TextSecondary,
        ),
        ActionWay::Applied => (
          "orchestrator.report.item.applied",
          Icon::CircleCheck,
          PaletteColor::Success,
        ),
      };
      let what = action
        .task
        .as_deref()
        .or(action.subject.as_deref())
        .unwrap_or(&action.tool);
      Item {
        glyph,
        tone,
        text: what.into(),
        meta: self.t(
          key,
          &[("tool", action.tool.clone()), ("time", display::clock(action.at))],
        ),
        tip: None,
        link: action.link.clone(),
      }
    });
    merged.chain(actions).collect()
  }

  /// The runs ready to merge, stopped with what the person can do, and at work.
  fn stands(&self, report: &DailyReport) -> Vec<Item> {
    let ready = report.ready.iter().map(|run| {
      self.run(
        run,
        (Icon::GitPullRequest, PaletteColor::Success),
        ("orchestrator.report.item.ready", None),
      )
    });
    let stopped = report.stopped.iter().map(|stopped| {
      let (look, tip) = stopped_look(stopped.end);
      self.run(&stopped.run, look, ("orchestrator.report.item.stopped", Some(tip)))
    });
    let working = report.working.iter().map(|run| {
      self.run(
        run,
        (Icon::Play, PaletteColor::TextSecondary),
        ("orchestrator.report.item.working", None),
      )
    });
    ready.chain(stopped).chain(working).collect()
  }

  fn run(&self, run: &ReportRun, (glyph, tone): (Icon, PaletteColor), (meta, tip): (&str, Option<&str>)) -> Item {
    let args = [
      ("run", run.number.to_string()),
      ("step", run.step.clone().unwrap_or_else(|| NONE.to_owned())),
    ];
    Item {
      glyph,
      tone,
      text: self.task(&run.task, &run.title),
      meta: self.t(meta, &args),
      tip: tip.map(|tip| self.t(tip, &args)),
      link: run.link.clone(),
    }
  }

  /// What waits for the person.
  fn waits(&self, report: &DailyReport) -> Vec<Item> {
    report
      .waiting
      .iter()
      .map(|wait| {
        // Without what it is about (a step not read, a proposal without words), its words say only what it is.
        let (said, bare, glyph) = match wait.kind {
          PersonWaitKind::Approval => (
            "orchestrator.report.item.wait.approval",
            "orchestrator.report.item.wait.approvalBare",
            Icon::ShieldAlert,
          ),
          PersonWaitKind::Question => (
            "orchestrator.report.item.wait.question",
            "orchestrator.report.item.wait.questionBare",
            Icon::Hand,
          ),
          PersonWaitKind::Message => (
            "orchestrator.report.item.wait.message",
            "orchestrator.report.item.wait.messageBare",
            Icon::MessageSquareText,
          ),
          PersonWaitKind::Plan => (
            "orchestrator.report.item.wait.plan",
            "orchestrator.report.item.wait.planBare",
            Icon::Map,
          ),
          PersonWaitKind::Proposal => (
            "orchestrator.report.item.wait.proposal",
            "orchestrator.report.item.wait.proposalBare",
            Icon::Hourglass,
          ),
        };
        let task = wait.task.clone().unwrap_or_else(|| NONE.to_owned());
        let meta = match (wait.kind, wait.number) {
          (PersonWaitKind::Proposal, _) => task.as_str().into(),
          (_, Some(number)) => self.t("orchestrator.report.item.run", &[("run", number.to_string())]),
          (_, None) => Arc::from(""),
        };
        let key = if wait.what.is_some() { said } else { bare };
        Item {
          glyph,
          tone: PaletteColor::Warning,
          text: self.t(key, &[("task", task), ("what", wait.what.clone().unwrap_or_default())]),
          meta,
          tip: None,
          link: wait.link.clone(),
        }
      })
      .collect()
  }

  /// The day's tokens and agent time against the daily budget, then by task and by account.
  fn usage(&self, report: &DailyReport) -> Vec<Item> {
    let usage = &report.usage;
    let spent = compact_tokens(usage.spent);
    let total = Item {
      glyph: Icon::Coins,
      tone: PaletteColor::TextMuted,
      text: match usage.budget {
        Some(budget) => self.t(
          "orchestrator.report.item.total",
          &[("spent", spent), ("budget", compact_tokens(budget))],
        ),
        None => self.t("orchestrator.report.item.totalUnlimited", &[("spent", spent)]),
      },
      meta: self.t(
        "orchestrator.report.item.agentTime",
        &[("time", self.active(usage.active_ms))],
      ),
      tip: None,
      link: usage.link.clone(),
    };
    let share = |spent: u64, ms: u64| {
      self.t(
        "orchestrator.report.item.share",
        &[("spent", compact_tokens(spent)), ("time", self.active(ms))],
      )
    };
    let tasks = usage.tasks.iter().map(|task| Item {
      glyph: Icon::Coins,
      tone: PaletteColor::TextMuted,
      text: self.task(&task.task, &task.title),
      meta: share(task.spent, task.active_ms),
      tip: None,
      link: task.link.clone(),
    });
    let providers = usage.providers.iter().map(|provider| Item {
      glyph: Icon::Bot,
      tone: PaletteColor::TextMuted,
      text: match &provider.by {
        SharedBy::Provider {
          name: Some(name),
          removed: false,
          ..
        } => name.as_str().into(),
        SharedBy::Provider {
          name: Some(name),
          removed: true,
          ..
        } => self.t("orchestrator.report.usage.removedProvider", &[("name", name.clone())]),
        SharedBy::Provider { name: None, .. } => self.ctx.t("orchestrator.report.usage.removedUnnamed"),
        SharedBy::Earlier => self.ctx.t("orchestrator.report.usage.earlier"),
        SharedBy::CommandTools => self.ctx.t("orchestrator.report.usage.commandTools"),
      },
      meta: share(provider.spent, provider.active_ms),
      tip: None,
      link: provider.link.clone(),
    });
    std::iter::once(total).chain(tasks).chain(providers).collect()
  }

  fn active(&self, ms: u64) -> String {
    message_text(self.ctx, &display::duration(Duration::from_millis(ms))).to_string()
  }

  /// The queued tasks, in the order they are queued.
  fn next(&self, report: &DailyReport) -> Vec<Item> {
    report
      .queued
      .iter()
      .enumerate()
      .map(|(index, task)| Item {
        glyph: Icon::ListOrdered,
        tone: PaletteColor::TextMuted,
        text: self.task(&task.task, &task.title),
        meta: self.t("orchestrator.report.item.next", &[("place", (index + 1).to_string())]),
        tip: None,
        link: task.link.clone(),
      })
      .collect()
  }
}

/// The icon and tone of a run that stopped so, and what its tooltip says the person can do.
const fn stopped_look(end: RunEnd) -> ((Icon, PaletteColor), &'static str) {
  let waits = (Icon::TriangleAlert, PaletteColor::Warning);
  match end {
    RunEnd::Succeeded => (
      (Icon::GitPullRequest, PaletteColor::Success),
      "orchestrator.report.stopped.ready",
    ),
    RunEnd::Failed => (
      (Icon::CircleX, PaletteColor::Danger),
      "orchestrator.report.stopped.failed",
    ),
    RunEnd::Stopped { why } => (
      waits,
      match why {
        StopReason::RetriesExhausted { .. } => "orchestrator.report.stopped.retries",
        StopReason::BrokeAgain => "orchestrator.report.stopped.brokeAgain",
        StopReason::NoRoute => "orchestrator.report.stopped.noRoute",
        StopReason::Unverified => "orchestrator.report.stopped.unverified",
        StopReason::Unchanged => "orchestrator.report.stopped.unchanged",
      },
    ),
    RunEnd::Limit { limit } => (
      waits,
      match limit {
        RunLimit::Tokens => "orchestrator.report.stopped.limitTokens",
        RunLimit::Time => "orchestrator.report.stopped.limitTime",
        RunLimit::DailyBudget => "orchestrator.report.stopped.limitDaily",
        RunLimit::GroupBudget => "orchestrator.report.stopped.limitGroup",
        RunLimit::StepBudget { .. } => "orchestrator.report.stopped.limitStep",
        RunLimit::PlanLimit { .. } => "orchestrator.report.stopped.limitPlan",
      },
    ),
    RunEnd::Interrupted => (
      (Icon::CirclePause, PaletteColor::Warning),
      "orchestrator.report.stopped.interrupted",
    ),
  }
}
