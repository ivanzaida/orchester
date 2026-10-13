//! What waits for a person in one project, one item per waiting object, as the title bar's Needs you panel
//! lists it (layer `Needs you` `eBhFg`) and every Needs you count counts it (the pill, the status bar, a tab's
//! mark): a tool request waiting for a decision, a question with suggested answers, a run waiting on a plan,
//! on an agent's question or on a decision after its retries, and a run recorded interrupted. A run that reads
//! interrupted only because no runner looks after it is none: a runner records it once one does. Approvals and
//! questions with suggested answers are answered in place, through their cards in the run's timeline
//! ([`WaitingKind::card_key`], [`InPlace`]); the others open their task.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::{RunId, TaskId},
  question::AskedQuestions,
  run::{RunNumber, TaskPhase},
  task::Task,
  time::Timestamp,
};
use orchester_store::StoreError;

use super::{
  approvals::ShownApproval,
  conversation::Conversation,
  data::ProjectTasks,
  detail::TaskDetail,
  list::{AwaitedDecision, AwaitedPlan, AwaitedQuestion, Latest, LatestRun},
  standing::CurrentStep,
  timeline::{
    self, Item, Sources, approval_key,
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
}

impl WaitingKind {
  /// The key of the card in its run's timeline that answers it in place: an approval's or a question's with
  /// suggested answers; `None` for what needs its task's page around it.
  pub fn card_key(&self) -> Option<Arc<str>> {
    match self {
      Self::Approval(shown) => Some(approval_key(&shown.approval.id)),
      Self::Choice(choice) => Some(card_key(&choice.request)),
      Self::Question(_) | Self::Plan(_) | Self::Decision(_) | Self::Interrupted { .. } => None,
    }
  }
}

/// One object waiting for the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingItem {
  /// Its task, while it can be read.
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
  /// Everything that waits for the person in the project, oldest first.
  pub fn waiting(&self) -> Vec<WaitingItem> {
    let mut items: Vec<WaitingItem> = self
      .pending_tool_approvals
      .iter()
      .map(|shown| self.approval_item(shown))
      .collect();
    items.extend(
      self
        .asked
        .iter()
        .filter(|asked| asked.waits())
        .filter_map(|asked| self.choice_item(asked)),
    );
    let answered_in_place: Vec<TaskId> = items.iter().filter_map(|item| Some(item.task.as_ref()?.id)).collect();
    items.extend(
      self
        .tasks
        .iter()
        .filter(|task| !answered_in_place.contains(&task.id()))
        .filter_map(|task| self.run_item(task)),
    );
    items.sort_by_key(|item| item.since);
    items
  }

  /// What *Needs you* counts: everything that waits for the person ([`Self::waiting`]). Whether a dialog or the
  /// panel is open changes nothing.
  pub fn needs_you(&self) -> usize {
    self.waiting().len()
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

  /// What `task`'s latest run waits for from the person, if anything.
  fn run_item(&self, task: &Task) -> Option<WaitingItem> {
    let Some(Latest::Run(latest)) = self.phases.get(&task.id()) else {
      return None;
    };
    let kind = match latest.phase {
      TaskPhase::Interrupted if !latest.lost_runner => WaitingKind::Interrupted {
        checkpoint: latest.checkpoint.clone(),
      },
      phase if phase.needs_you() => match (&latest.plan, &latest.decision) {
        (Some(plan), _) => WaitingKind::Plan(plan.clone()),
        (None, Some(decision)) => WaitingKind::Decision(decision.clone()),
        (None, None) => WaitingKind::Question(latest.question.clone()),
      },
      _ => return None,
    };
    Some(WaitingItem {
      task: Some(WaitingTask::of(task)),
      run: Some((latest.id, latest.number)),
      step: latest.step.clone(),
      since: latest.updated_at,
      kind,
    })
  }
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
}
