//! The rules of the daily report (ORC-47) in one place: which of the day's facts ([`DayFacts`]) it tells and where,
//! whether the day had any activity, and which days' reports are due. Nothing here reads the database or invents a
//! fact: a run is told as it stands in its record, an action as its entry kept it.
//!
//! - *Merged*: the tasks whose run Orchester merged that day, in the order they were merged.
//! - *Ready*, *stopped*, *at work*: each task's latest run as it stands when the report is read: succeeded and not
//!   merged; failed, stopped at a limit, waiting for a decision or interrupted, with how; or running. A pause the
//!   person asked for and a cancel are neither.
//! - *Queued*: the queued tasks whose run has not started, in the order the project lists them.
//! - *Waits for the person* ([`super::waiting`]), *usage* ([`super::usage`]).
//! - *The Orchestrator's actions*: what it did itself that day (done, never settled by a person), and what a person
//!   applied of its proposals that day (applied, or a proposed start the person started), each at when it was done.
//! - A day is active when a run was merged, ended or stopped, the Orchestrator acted or its conversations kept anything
//!   but a report, or tokens or agent time were spent; a day without activity gets no report.

use chrono::{Days, NaiveDate};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{
    ActionOutcome, ChatAction, DailyReport, EntryKind, ReportSchedule, RunEnd, StopReason,
    change::{Change, Publishing, StepName},
    report::{ActionWay, MergedTask, ReportAction, ReportRun, ReportTask, StoppedRun},
  },
  project::TaskKey,
  run::{PauseReason, Run, RunStatus},
  task::{Task, TaskStatus},
  time::Timestamp,
  usage::{day_start, local_date},
};

use super::{
  facts::{DayFacts, TaskFacts},
  usage, waiting,
};
use crate::{
  links::{Link, TaskPage},
  orchestrator::run_ends::stopped_step,
};

/// How many days back a report that did not come, because the app was closed at its time, still comes: today's and
/// the six before.
pub const CATCH_UP_DAYS: u64 = 7;

/// A local day, from its midnight up to the next.
#[derive(Clone, Copy)]
pub(super) struct Day {
  start: Timestamp,
  end: Timestamp,
}

impl Day {
  pub(super) fn of(day: NaiveDate) -> Self {
    let next = day.checked_add_days(Days::new(1)).unwrap_or(day);
    Self {
      start: day_start(day),
      end: day_start(next),
    }
  }

  pub(super) fn holds(self, at: Timestamp) -> bool {
    self.start <= at && at < self.end
  }
}

/// The report of `facts`' day in project `project`.
pub(super) fn build(facts: &DayFacts, project: ProjectId) -> DailyReport {
  let day = Day::of(facts.day);
  let mut report = DailyReport {
    day: facts.day,
    built: facts.now,
    merged: merged(facts, project, day),
    ready: Vec::new(),
    stopped: Vec::new(),
    working: Vec::new(),
    waiting: waiting::of(facts, project),
    actions: actions(facts, project, day),
    usage: usage::of(facts, project),
    queued: Vec::new(),
    automatic: false,
  };
  for task in &facts.tasks {
    place(&mut report, facts, (project, task));
  }
  report
}

/// Puts `task`'s latest run, or the task while its run has not started, where it stands.
fn place(report: &mut DailyReport, facts: &DayFacts, (project, task): (ProjectId, &TaskFacts)) {
  let Some(run) = task.latest() else {
    if task.task.status() == TaskStatus::Queued {
      report.queued.push(report_task(project, &task.task));
    }
    return;
  };
  let told = |step: Option<String>, since: Option<Timestamp>| ReportRun {
    task: task.task.key().to_string(),
    title: task.task.details().title.as_str().to_owned(),
    run: run.id(),
    number: run.number().get(),
    step,
    since,
    link: Link::Task(project, task.task.key().clone(), TaskPage::Summary).path(),
  };
  let ended = facts.ended.get(&run.id()).copied();
  match run.status() {
    RunStatus::Pending => report.queued.push(report_task(project, &task.task)),
    RunStatus::Running => report.working.push(told(None, None)),
    RunStatus::Succeeded if !task.merged(run.id()) => report.ready.push(told(None, ended)),
    RunStatus::Succeeded | RunStatus::Cancelled | RunStatus::Paused(PauseReason::Requested) => {}
    _ => {
      if let Some(end) = end_of(run) {
        let step = stopped_step(run)
          .and_then(|key| StepName::in_run(run, key))
          .map(|step| step.name);
        report.stopped.push(StoppedRun {
          run: told(step, ended),
          end,
        });
      }
    }
  }
}

/// How `run` stopped, while it stands stopped: failed, waiting for a decision, at a limit or interrupted.
fn end_of(run: &Run) -> Option<RunEnd> {
  match run.status() {
    RunStatus::Failed(_) => Some(RunEnd::Failed),
    RunStatus::WaitingForPerson(_) => StopReason::of_run(run).map(|why| RunEnd::Stopped { why }),
    RunStatus::Paused(PauseReason::LimitReached(limit)) => Some(RunEnd::Limit { limit: *limit }),
    RunStatus::Interrupted(_) => Some(RunEnd::Interrupted),
    _ => None,
  }
}

fn report_task(project: ProjectId, task: &Task) -> ReportTask {
  ReportTask {
    task: task.key().to_string(),
    title: task.details().title.as_str().to_owned(),
    link: Link::Task(project, task.key().clone(), TaskPage::Details).path(),
  }
}

/// The tasks whose run Orchester merged on `day`, in the order they were merged.
fn merged(facts: &DayFacts, project: ProjectId, day: Day) -> Vec<MergedTask> {
  let mut merged: Vec<MergedTask> = facts
    .tasks
    .iter()
    .flat_map(|task| {
      task
        .merges
        .iter()
        .filter(move |merge| day.holds(merge.merged_at))
        .filter_map(move |merge| {
          let run = task.runs.iter().find(|run| run.id() == merge.run)?;
          Some(MergedTask {
            task: task.task.key().to_string(),
            title: task.task.details().title.as_str().to_owned(),
            number: run.number().get(),
            at: merge.merged_at,
            link: Link::Task(project, task.task.key().clone(), TaskPage::Summary).path(),
          })
        })
    })
    .collect();
  merged.sort_by_key(|task| task.at);
  merged
}

/// What the Orchestrator did on `day`, itself or applied by the person, in the order it was done.
fn actions(facts: &DayFacts, project: ProjectId, day: Day) -> Vec<ReportAction> {
  let mut actions: Vec<ReportAction> = facts
    .chats
    .iter()
    .flat_map(|(chat, entries)| entries.iter().map(move |entry| (*chat, entry)))
    .filter_map(|(chat, entry)| {
      let EntryKind::Action(action) = &entry.kind else {
        return None;
      };
      let (way, at) = carried(action, entry.at)?;
      day.holds(at).then(|| ReportAction {
        tool: action.tool.clone(),
        task: action.task.clone(),
        subject: action.change.as_ref().map(|change| change.subject.clone()),
        way,
        at,
        link: action_link(project, chat, action),
      })
    })
    .collect();
  actions.sort_by_key(|action| action.at);
  actions
}

/// How `action`, kept at `kept`, was carried out, and when: by the Orchestrator itself, or applied by a person when
/// they settled it; `None` for one that was not carried out (proposed, refused or dismissed).
fn carried(action: &ChatAction, kept: Timestamp) -> Option<(ActionWay, Timestamp)> {
  match (&action.outcome, action.settled) {
    (ActionOutcome::Done, None) => Some((ActionWay::Direct, kept)),
    (ActionOutcome::Done | ActionOutcome::Applied, settled) => Some((ActionWay::Applied, settled.unwrap_or(kept))),
    _ => None,
  }
}

/// Where `action` of conversation `chat` links: its task's Summary, the resource it published, or else its
/// conversation.
fn action_link(project: ProjectId, chat: ChatId, action: &ChatAction) -> String {
  if let Some(key) = action.task.as_deref().and_then(|key| key.parse::<TaskKey>().ok()) {
    return Link::Task(project, key, TaskPage::Summary).path();
  }
  let published = action.change.as_ref().and_then(|change| match &change.change {
    Change::Publish { of, resource, .. } => Some(match of {
      Publishing::Pipeline => Link::Pipeline(project, *resource),
      Publishing::Profile => Link::Profile(project, *resource),
      Publishing::Instruction => Link::Instructions(project),
      Publishing::Skill => Link::Skill(project, *resource),
      Publishing::Tool => Link::Tool(project, *resource),
    }),
    _ => None,
  });
  published.unwrap_or(Link::Orchestrator(project, chat)).path()
}

/// Whether `facts`' day had any activity, as `report` of it tells and its conversations kept.
pub(super) fn active(facts: &DayFacts, report: &DailyReport) -> bool {
  let day = Day::of(facts.day);
  let kept = facts
    .chats
    .iter()
    .flat_map(|(_, entries)| entries)
    .any(|entry| day.holds(entry.at) && !matches!(entry.kind, EntryKind::Report { .. }));
  !report.merged.is_empty()
    || !report.actions.is_empty()
    || report.usage.spent > 0
    || report.usage.active_ms > 0
    || facts.ended.values().any(|at| day.holds(*at))
    || kept
}

/// The days whose report is due at `now` under `schedule`, oldest first: of today and the days before it within
/// [`CATCH_UP_DAYS`], those whose report time has come and came after the report was turned on. Whether one was kept
/// already is the caller's to read.
pub fn due_days(schedule: ReportSchedule, now: Timestamp) -> Vec<NaiveDate> {
  let today = local_date(now);
  (0..CATCH_UP_DAYS)
    .rev()
    .filter_map(|back| today.checked_sub_days(Days::new(back)))
    .filter(|day| {
      let at = schedule.time.on(*day);
      at <= now && at > schedule.since
    })
    .collect()
}
