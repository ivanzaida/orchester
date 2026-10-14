//! What waits for a person in one project, one item per waiting object, as the title bar's Needs you panel
//! lists it (layer `Needs you` `eBhFg`, note `zJR9O`) and every Needs you count counts it (the pill, the status
//! bar, a tab's mark): a tool request waiting for a decision (also one whose task cannot be read), a question
//! with suggested answers, each step of a run waiting on a plan or on an agent's question, a decision after a
//! step's retries, a run stopped at its limit, a run whose every step passed and whose work waits to be merged
//! (not once its task is Done or Canceled, its work reached its base branch without Orchester, or it committed
//! nothing to merge, `tasks::delivery`), a run
//! recorded interrupted, and the project's memory records waiting for approval, together one item. A run
//! that reads interrupted only because no runner looks after it is none: a runner records it once one does.
//! A merge that stopped at conflicts is known only to the session that tried it, so it is not counted here.
//! Approvals and questions with suggested answers are answered in place, through their cards in the run's
//! timeline ([`WaitingKind::card_key`], [`InPlace`]); the others open their task, or the project's memory.

use std::{
  collections::HashSet,
  path::Path,
  sync::{Arc, OnceLock},
};

use orchester_core::{
  id::{RunId, StepExecutionId, TaskId},
  question::{Answer, AskedQuestions, QuestionState},
  run::{Answerer, DecidedBy, RunNumber, TaskPhase, ToolApprovalDecision},
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_store::StoreError;

use super::{
  approvals::ShownApproval,
  conversation::Conversation,
  data::ProjectTasks,
  detail::TaskDetail,
  list::{AwaitedDecision, AwaitedPlan, AwaitedQuestion, Latest, LatestRun, StepWait},
  standing::CurrentStep,
  timeline::{
    self, Item, ItemKind, Sources, approval_key,
    load::{Requests, requests_in},
    question::card_key,
  },
};

/// The task a waiting object belongs to, as people name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingTask {
  pub id: TaskId,
  pub key: Arc<str>,
  pub title: Arc<str>,
}

impl WaitingTask {
  fn of(task: &Task) -> Self {
    Self {
      id: task.id(),
      key: task.key().to_string().into(),
      title: task.details().title.as_str().into(),
    }
  }
}

/// A question with suggested answers waiting for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChoiceWaiting {
  /// The request's id in its run.
  pub request: Arc<str>,
  /// The asking agent's role.
  pub agent: Arc<str>,
  /// What its first question asks.
  pub first: Arc<str>,
  /// How many questions it asks.
  pub count: usize,
}

/// What a waiting object is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaitingKind {
  /// A tool request waiting for a decision.
  Approval(Box<ShownApproval>),
  /// A question with suggested answers.
  Choice(ChoiceWaiting),
  /// An agent's question in the conversation, answered there; `None` when the run waits for a message without
  /// one that can be read.
  Question(Option<AwaitedQuestion>),
  Plan(AwaitedPlan),
  /// A decision after the step's retries ran out or it broke again.
  Decision(AwaitedDecision),
  /// A run recorded interrupted, with its last checkpoint.
  Interrupted {
    checkpoint: Option<Arc<str>>,
  },
  /// A run that reached its token or time limit, or the project's daily budget, and waits for it to be raised.
  StoppedAtLimit {
    daily_budget: bool,
  },
  /// A run whose every step passed: its work waits for review and to be merged.
  ReadyToMerge,
  /// The project's memory records waiting for a person's approval, this many.
  Memory {
    count: usize,
  },
}

impl WaitingKind {
  /// The key of the card in its run's timeline that answers it in place: an approval's or a question's with
  /// suggested answers; `None` for what needs its task's page around it.
  pub fn card_key(&self) -> Option<Arc<str>> {
    match self {
      Self::Approval(shown) => Some(approval_key(&shown.approval.id)),
      Self::Choice(choice) => Some(card_key(&choice.request)),
      Self::Question(_)
      | Self::Plan(_)
      | Self::Decision(_)
      | Self::Interrupted { .. }
      | Self::StoppedAtLimit { .. }
      | Self::ReadyToMerge
      | Self::Memory { .. } => None,
    }
  }
}

/// What waits in a project ([`ProjectTasks::waiting`]), worked out from the rest of its [`ProjectTasks`] the first
/// time it is asked for and kept with it, so the pill, a tab's mark and the panel do not work it out again in each
/// render. A reading of the tasks starts without it, and so does every [`ProjectTasks`] made from another
/// ([`ProjectTasks::without_runner`], [`ProjectTasks::not_shared_view`]).
#[derive(Clone, Default)]
pub struct WaitingOnce(Arc<OnceLock<Arc<[WaitingItem]>>>);

/// It follows from the other fields, so it never tells two [`ProjectTasks`] apart.
impl PartialEq for WaitingOnce {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl Eq for WaitingOnce {}

impl std::fmt::Debug for WaitingOnce {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str("WaitingOnce")
  }
}

/// One object waiting for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingItem {
  /// Its task, while it can be read; none for the project's memory.
  pub task: Option<WaitingTask>,
  /// Its run and the run's number, while they can be read.
  pub run: Option<(RunId, RunNumber)>,
  /// The step its run is on or stopped at.
  pub step: Option<CurrentStep>,
  /// Since when it waits.
  pub since: Timestamp,
  pub kind: WaitingKind,
}

impl ProjectTasks {
  /// Everything that waits for the person in the project, oldest first: what the Needs you panel lists and every
  /// Needs you counts ([`Self::needs_you`]). Worked out once for these tasks ([`WaitingOnce`]).
  pub fn waiting(&self) -> Arc<[WaitingItem]> {
    self
      .waiting_once
      .0
      .get_or_init(|| self.work_out_waiting().into())
      .clone()
  }

  /// What every *Needs you* counts — the pill, the status bar, a tab's mark, the Overview's figure, column and
  /// tab, the task list's tab and All projects: everything that waits for the person ([`Self::waiting`]).
  /// Whether a dialog or the panel is open changes nothing.
  pub fn needs_you(&self) -> usize {
    self.waiting_once.0.get_or_init(|| self.work_out_waiting().into()).len()
  }

  /// The tasks something of waits for the person ([`Self::waiting`]): those a *Needs you* tab lists.
  pub fn waiting_tasks(&self) -> HashSet<TaskId> {
    self
      .waiting()
      .iter()
      .filter_map(|item| item.task.as_ref().map(|task| task.id))
      .collect()
  }

  fn work_out_waiting(&self) -> Vec<WaitingItem> {
    let mut items: Vec<WaitingItem> = self
      .pending_tool_approvals
      .iter()
      .map(|shown| self.approval_item(shown))
      .collect();
    items.extend(self.waiting_choices().filter_map(|asked| self.choice_item(asked)));
    for task in self.tasks.iter() {
      self.run_items(task, &mut items);
    }
    if let Some(oldest) = self.proposed_memory.iter().min() {
      items.push(WaitingItem {
        task: None,
        run: None,
        step: None,
        since: *oldest,
        kind: WaitingKind::Memory {
          count: self.proposed_memory.len(),
        },
      });
    }
    items.sort_by_key(|item| item.since);
    items
  }

  fn task(&self, id: TaskId) -> Option<&Task> {
    self.tasks.iter().find(|task| task.id() == id)
  }

  /// The latest run of a task that is `run`, with its task.
  fn latest_of_run(&self, run: RunId) -> Option<(&Task, &LatestRun)> {
    self.phases.iter().find_map(|(task, latest)| {
      let latest = latest.run().filter(|latest| latest.id == run)?;
      Some((self.task(*task)?, latest))
    })
  }

  fn approval_item(&self, shown: &ShownApproval) -> WaitingItem {
    let latest = self.latest_of_run(shown.approval.run);
    let task = shown.task_id.and_then(|id| self.task(id)).map(WaitingTask::of);
    WaitingItem {
      task,
      run: shown.run.map(|number| (shown.approval.run, number)),
      step: latest.and_then(|(_, latest)| latest.step.clone()),
      since: shown.approval.requested_at,
      kind: WaitingKind::Approval(Box::new(shown.clone())),
    }
  }

  /// A waiting question with suggested answers of a task's latest run.
  fn choice_item(&self, asked: &AskedQuestions) -> Option<WaitingItem> {
    let request = &asked.request;
    let (task, latest) = self.latest_of_run(request.run())?;
    Some(WaitingItem {
      task: Some(WaitingTask::of(task)),
      run: Some((latest.id, latest.number)),
      step: latest.step.clone(),
      since: request.asked_at(),
      kind: WaitingKind::Choice(ChoiceWaiting {
        request: request.id().into(),
        agent: request.agent().into(),
        first: request
          .questions()
          .first()
          .map_or_else(|| "".into(), |question| question.text().into()),
        count: request.questions().len(),
      }),
    })
  }

  fn waiting_choices(&self) -> impl Iterator<Item = &AskedQuestions> {
    self.asked.iter().filter(|asked| asked.waits())
  }

  /// Adds to `items` what `task`'s latest run waits for from the person besides its tool requests and its
  /// questions with suggested answers: being recorded interrupted, stopped at its limit or ready to merge, a
  /// decision after its retries, and each step's plan and question, which a run stopped at its limit or
  /// recorded interrupted keeps waiting. A step that ended with a question with suggested answers is that
  /// question, listed once. A run that waits for a person without anything that can be read is one item still.
  fn run_items(&self, task: &Task, items: &mut Vec<WaitingItem>) {
    let Some(Latest::Run(latest)) = self.phases.get(&task.id()) else {
      return;
    };
    let item = |step: Option<CurrentStep>, kind| WaitingItem {
      task: Some(WaitingTask::of(task)),
      run: Some((latest.id, latest.number)),
      step,
      since: latest.updated_at,
      kind,
    };
    let settled = match latest.phase {
      TaskPhase::Interrupted if !latest.lost_runner => Some(WaitingKind::Interrupted {
        checkpoint: latest.checkpoint.clone(),
      }),
      TaskPhase::StoppedAtLimit => Some(WaitingKind::StoppedAtLimit {
        daily_budget: latest.daily_budget,
      }),
      TaskPhase::Succeeded if !latest.nothing_to_merge && !settled_by_person(task) => Some(WaitingKind::ReadyToMerge),
      _ => None,
    };
    let before = items.len();
    if let Some(kind) = settled {
      items.push(item(latest.step.clone(), kind));
    } else if !latest.phase.needs_you() {
      return;
    }
    if let Some(decision) = &latest.decision {
      items.push(item(latest.step.clone(), WaitingKind::Decision(decision.clone())));
    }
    for waiting in &latest.waiting {
      let kind = match &waiting.wait {
        StepWait::Plan(plan) => WaitingKind::Plan(plan.clone()),
        StepWait::Answer(_) if self.chosen_at(latest.id, waiting.execution) => continue,
        StepWait::Answer(question) => WaitingKind::Question(question.clone()),
      };
      items.push(item(waiting.step.clone(), kind));
    }
    let asks_elsewhere = self
      .pending_tool_approvals
      .iter()
      .any(|shown| shown.approval.run == latest.id)
      || self.waiting_choices().any(|asked| asked.request.run() == latest.id);
    if items.len() == before && !asks_elsewhere {
      items.push(item(latest.step.clone(), WaitingKind::Question(None)));
    }
  }

  /// Whether `execution` of `run` waits on a question with suggested answers, which is listed for it.
  fn chosen_at(&self, run: RunId, execution: StepExecutionId) -> bool {
    self
      .waiting_choices()
      .any(|asked| asked.request.run() == run && asked.request.execution() == execution)
  }
}

/// Whether the person settled `task` themselves, Done or Canceled, so its work waits to be merged no more.
fn settled_by_person(task: &Task) -> bool {
  matches!(
    task.organization().status(),
    WorkflowStatus::Done | WorkflowStatus::Canceled
  )
}

/// What answering an item in place reads: its task with its latest run, and that run's requests. The card it
/// answers through is the conversation's own ([`Self::card`]), so the panel answers by the same rules.
#[derive(Clone, Debug, PartialEq)]
pub struct InPlace {
  pub detail: Arc<TaskDetail>,
  pub requests: Requests,
}

impl InPlace {
  /// Reads `task` of the project at `repo_root`, and its latest run's requests.
  pub fn read(repo_root: &Path, task: TaskId) -> Result<Self, StoreError> {
    let detail = TaskDetail::load_in(repo_root, task)?;
    let requests = match &detail.run {
      Some(run) => requests_in(repo_root, run.id())?,
      None => Requests::default(),
    };
    Ok(Self {
      detail: Arc::new(detail),
      requests,
    })
  }

  /// The card keyed `key` in the latest run's timeline, as the conversation shows it while its run `lost` its
  /// runner or not; `None` when the run has no such card (the request belongs to an earlier run).
  pub fn card(&self, key: &str, lost: bool) -> Option<Item> {
    let run = self.detail.run.as_ref()?;
    let conversation = Conversation::of(&self.detail, lost, None);
    timeline::build(&Sources {
      run,
      entries: &[],
      conversation: &conversation,
      pending: &self.requests.pending,
      decided: &self.requests.decided,
      questions: &self.requests.questions,
      merge: self.detail.merge.as_ref(),
    })
    .into_iter()
    .find(|item| *item.key == *key)
  }

  /// How the request keyed `key` went once it no longer waits, as the panel's note says it (`FETWe`), while
  /// its run `lost` its runner or not.
  pub fn settled(&self, key: &str, lost: bool) -> Settled {
    let Some(item) = self.card(key, lost) else {
      return Settled::Unknown;
    };
    match item.kind {
      ItemKind::Approval(card) => match card.decided {
        Some(Some(ToolApprovalDecision::Approved)) => Settled::Approved { by: card.decided_by },
        Some(Some(ToolApprovalDecision::Denied)) => Settled::Denied { by: card.decided_by },
        Some(None) => Settled::Ended { at: card.decided_at },
        None if lost => Settled::Lost,
        None => Settled::Unknown,
      },
      ItemKind::Choice(card) => match card.asked.state {
        QuestionState::Answered { answers, by, at } => Settled::Answered {
          answer: answers.into_iter().next(),
          by,
          at,
        },
        QuestionState::Interrupted { at } => Settled::Ended { at: Some(at) },
        QuestionState::Waiting if lost => Settled::Lost,
        QuestionState::Waiting => Settled::Unknown,
      },
      _ => Settled::Unknown,
    }
  }
}

/// How a request answered in place went, once it no longer waits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Settled {
  /// A tool request approved, by whom when that was recorded.
  Approved { by: Option<DecidedBy> },
  /// A tool request denied, by whom when that was recorded.
  Denied { by: Option<DecidedBy> },
  /// A question answered `by` at `at`; `answer` is its first question's.
  Answered {
    answer: Option<Answer>,
    by: Answerer,
    at: Timestamp,
  },
  /// The asking agent stopped waiting before an answer reached it, at `at` when that was recorded.
  Ended { at: Option<Timestamp> },
  /// Its run lost its runner: no agent waits for the answer now.
  Lost,
  /// It no longer waits, and what was read does not say how yet: it was settled after the read, or belongs
  /// to an earlier run.
  Unknown,
}
