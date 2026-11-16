//! A local module with a branch of its own on its way to its base branch (`orchester_core::module::delivery`,
//! `docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`): the module's own branch and its settings
//! ([`change_own_in`]), what its delivery waits for as the project's records tell it ([`facts`]), and the delivery
//! itself ([`deliver_in`]): the merge a team module's delivery makes ([`merge_module_in`]), of the module's branch into
//! its base branch, never while a run of the module still works. A team module's branch and delivery are its
//! server's (`crate::team_work`); the rule that says what a module waits for is one for both.

use std::path::Path;

use orchester_core::{
  id::{ModuleId, ResourceId, TaskId},
  integration::{
    MergeError, ModuleDelivery,
    base::{BaseChoice, RunBaseError, branch_choices, set_branch_tip},
    checked_out_branch, is_ancestor,
  },
  module::{
    Module, ModuleError,
    delivery::{Acceptance, DeliveryFacts, DeliveryStep, ModuleDelivered, OwnBranch, own_branch_name},
  },
  run::{BranchName, TaskPhase},
  store::{MergeStore, RunStore, TaskStore},
  task::{Task, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, StoreError};

use crate::{
  messages::{self, Message},
  tasks::{
    changes::GIT,
    detail::run_store_error,
    module_delivery::{ModuleMergeError, merge_module_in, module_merged_in},
  },
};

/// What a module form or a tool changes of a module's own branch; what is `None` stays as it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnChange {
  /// Whether the module has a branch of its own.
  pub own_branch: Option<bool>,
  /// The pipeline its acceptance runs on; `Some(None)` takes it away.
  pub acceptance: Option<Option<ResourceId>>,
  pub deliver_when_accepted: Option<bool>,
}

impl OwnChange {
  pub const fn is_empty(&self) -> bool {
    self.own_branch.is_none() && self.acceptance.is_none() && self.deliver_when_accepted.is_none()
  }
}

/// Applies `change` to `module` of the project at `root`. A module that gets a branch of its own is given its name,
/// `module/<slug>`, one no other module and no branch of the repository has; the branch itself is made when the
/// module's first task starts. Taking the branch away leaves it in the repository: nothing deletes work. An
/// acceptance pipeline or *Deliver when accepted* without a branch of its own is refused, and so is taking the
/// branch away while it holds merged work that was not delivered. Blocks on SQLite and git.
pub fn change_own_in(root: &Path, programs: &Programs, module: &Module, change: &OwnChange) -> Result<Module, Message> {
  if change.is_empty() {
    return Ok(module.clone());
  }
  let stored = |error: StoreError| messages::store_error(&error);
  let mut db = ProjectDb::open(root).map_err(stored)?;
  if change.own_branch == Some(false) && module.own_branch().is_some() {
    let tasks = db.tasks().map_err(stored)?;
    let view = DeliveryView::read(&db, module, &tasks).map_err(stored)?;
    if view.is_some_and(|view| view.undelivered()) {
      return Err(Message::new("modules.ownBranch.holdsWork"));
    }
  }
  let own = match (change.own_branch, module.own_branch()) {
    (Some(false), _) => None,
    (_, Some(own)) => Some(own.clone()),
    (Some(true), None) => Some(OwnBranch::new(new_branch(&db, (root, programs), module)?)),
    (None, None) => None,
  };
  let own = match own {
    Some(own) => Some(OwnBranch {
      acceptance: change.acceptance.unwrap_or(own.acceptance),
      deliver_when_accepted: change.deliver_when_accepted.unwrap_or(own.deliver_when_accepted),
      ..own
    }),
    None
      if change.acceptance.is_some_and(|pipeline| pipeline.is_some()) || change.deliver_when_accepted == Some(true) =>
    {
      return Err(Message::new("modules.ownBranch.required"));
    }
    None => None,
  };
  db.set_module_own_branch(module.id(), own.as_ref()).map_err(stored)
}

/// The name of the branch `module` gets: free among the modules' own branches and the repository's local branches.
fn new_branch(db: &ProjectDb, (root, programs): (&Path, &Programs), module: &Module) -> Result<BranchName, Message> {
  let taken: Vec<BranchName> = db
    .modules()
    .map_err(|error| messages::store_error(&error))?
    .iter()
    .filter_map(|other| other.own_branch().map(|own| own.branch.clone()))
    .collect();
  let choices = branch_choices(&programs.runner(), Path::new(GIT), root).map_err(|error| {
    tracing::warn!(%error, "cannot read the repository's branches for a module's own branch");
    Message::new("modules.ownBranch.unreadable")
  })?;
  own_branch_name(module.name(), |branch| {
    taken.contains(branch) || choices.has_local(branch)
  })
  .ok_or_else(|| Message::new("modules.ownBranch.unreadable"))
}

/// What the delivery of `module` waits for once its every task is done, as the records of `db` tell it: its
/// acceptances and whether the latest stands for the branch as it is now, and whether the branch was delivered since
/// the last merge into it. Blocks on SQLite.
pub fn facts(db: &ProjectDb, module: &Module) -> Result<DeliveryFacts, StoreError> {
  let own = module.own_branch();
  let acceptances = db.module_acceptances(module.id())?;
  let is_acceptance = |task: TaskId| acceptances.iter().any(|(acceptance, _)| *acceptance == task);
  let tasks: Vec<Task> = db
    .tasks()?
    .into_iter()
    .filter(|task| task.module() == Some(module.id()))
    .collect();
  // When work last reached the module's branch: by any task, and by a task that is no acceptance.
  let (mut merged, mut work_merged) = (None, None);
  for task in &tasks {
    let last = last_merge(db, task.id())?;
    merged = merged.max(last);
    if !is_acceptance(task.id()) {
      work_merged = work_merged.max(last);
    }
  }
  let acceptance = match acceptances.last() {
    None => Acceptance::None,
    Some(&(task, at)) => match tasks.iter().find(|listed| listed.id() == task) {
      // An acceptance task that is gone decided nothing.
      None => Acceptance::NotPassed,
      Some(task) => acceptance_of(db, task, work_merged.is_some_and(|merged| merged > at))?,
    },
  };
  let delivered = match (db.module_delivery(module.id())?, merged) {
    (_, None) => true,
    (Some(delivery), Some(merged)) => delivery.at >= merged,
    (None, Some(_)) => false,
  };
  Ok(DeliveryFacts {
    own_branch: own.is_some(),
    acceptance_pipeline: own.is_some_and(|own| own.acceptance.is_some()),
    deliver_when_accepted: own.is_some_and(|own| own.deliver_when_accepted),
    acceptances: acceptances.len(),
    acceptance,
    delivered,
  })
}

/// When a run of task `task` was last merged; `None` while none was.
fn last_merge(db: &ProjectDb, task: TaskId) -> Result<Option<Timestamp>, StoreError> {
  let mut last = None;
  for run in db.runs_of_task(task).map_err(run_store_error)?.iter().flatten() {
    last = last.max(db.run_merge(run.id())?.map(|merge| merge.merged_at));
  }
  Ok(last)
}

/// Where the acceptance `task` is stands; `stale`: work was merged into the module's branch after it was made.
fn acceptance_of(db: &ProjectDb, task: &Task, stale: bool) -> Result<Acceptance, StoreError> {
  let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
  let latest = runs.iter().rev().find_map(|listed| listed.as_ref().ok());
  let merge = match latest {
    Some(run) => db.run_merge(run.id())?,
    None => None,
  };
  let status = task.organization().status();
  let phase = TaskPhase::of(task, latest, merge.as_ref());
  Ok(match (status, phase) {
    (WorkflowStatus::Canceled, _) | (_, TaskPhase::Failed | TaskPhase::Cancelled) => Acceptance::NotPassed,
    (WorkflowStatus::Done, _) | (_, TaskPhase::Merged) if stale => Acceptance::NotPassed,
    (WorkflowStatus::Done, _) | (_, TaskPhase::Merged) => Acceptance::Passed,
    _ => Acceptance::UnderWay,
  })
}

/// Why a module was not delivered.
#[derive(Debug)]
pub enum DeliverRefusal {
  /// The module has no branch of its own: its tasks merge into its base branch themselves.
  NoOwnBranch,
  /// These tasks of the module have a run that has not ended, by their keys.
  RunsAtWork(Vec<String>),
  /// Nothing was merged into the module's branch that its base branch does not have.
  NothingToDeliver,
  /// The checkout has no branch checked out, and the module names no base branch.
  NoBaseBranch,
  /// The module's own branch is the branch it would be delivered into.
  OnOwnBranch,
  /// Another merge or a local integration holds the checkout now.
  Busy,
  /// The merge conflicts with the base branch in these files.
  Conflict(Vec<String>),
  /// The person's checkout has uncommitted changes the delivery would overwrite, in these files.
  InTheWay(Vec<String>),
  /// Git did not merge it for another reason.
  Failed(String),
  Store(StoreError),
}

impl DeliverRefusal {
  /// The code a tool refuses with.
  pub const fn code(&self) -> &'static str {
    match self {
      Self::NoOwnBranch => "no_own_branch",
      Self::RunsAtWork(_) => "runs_at_work",
      Self::NothingToDeliver => "nothing_to_deliver",
      Self::NoBaseBranch => "no_base_branch",
      Self::OnOwnBranch => "module_branch_checked_out",
      Self::Busy => "integration_running",
      Self::Conflict(_) => "delivery_conflicts",
      Self::InTheWay(_) => "uncommitted_in_the_way",
      Self::Failed(_) => "delivery_failed",
      Self::Store(_) => "store_failed",
    }
  }

  /// What a person or the chat's agent reads of it.
  pub fn message(&self) -> Message {
    match self {
      Self::NoOwnBranch => Message::new("modules.delivery.noOwnBranch"),
      Self::RunsAtWork(tasks) => Message::new("modules.delivery.runsAtWork").with("tasks", tasks.join(", ")),
      Self::NothingToDeliver => Message::new("modules.delivery.nothing"),
      Self::NoBaseBranch => Message::new("modules.delivery.noBaseBranch"),
      Self::OnOwnBranch => Message::new("modules.delivery.onOwnBranch"),
      Self::Busy => Message::new("modules.delivery.busy"),
      Self::Conflict(files) => Message::new("modules.delivery.conflicts").with("files", files.join(", ")),
      Self::InTheWay(files) => Message::new("modules.delivery.inTheWay").with("files", files.join(", ")),
      Self::Failed(detail) => Message::new("modules.delivery.failed").with("detail", detail.clone()),
      Self::Store(error) => messages::store_error(error),
    }
  }
}

impl From<StoreError> for DeliverRefusal {
  fn from(error: StoreError) -> Self {
    Self::Store(error)
  }
}

/// Delivers `module` of the project at `root`: merges its own branch, at its tip, into its base branch (the branch
/// the checkout has checked out when the module names none), with a merge commit, and keeps the delivery. Refused
/// while a run of the module has not ended, when there is nothing to deliver, and as the merge refuses: a conflict
/// and a checkout that stands in the way leave everything as it was, for the person. A merge an earlier call made and
/// did not record is found, never made twice. Blocks on SQLite and git.
pub fn deliver_in(root: &Path, programs: &Programs, module: ModuleId) -> Result<ModuleDelivered, DeliverRefusal> {
  let mut db = ProjectDb::open(root)?;
  let module = db
    .module(module)?
    .ok_or_else(|| DeliverRefusal::Store(ModuleError::NotFound(module).into()))?;
  let own = module.own_branch().ok_or(DeliverRefusal::NoOwnBranch)?;
  let at_work = at_work(&db, module.id())?;
  if !at_work.is_empty() {
    return Err(DeliverRefusal::RunsAtWork(at_work));
  }
  let (runner, git) = (programs.runner(), Path::new(GIT));
  let unreadable = |error: &dyn std::fmt::Display| DeliverRefusal::Failed(error.to_string());
  let head = match set_branch_tip(&runner, git, (root, &own.branch)) {
    Ok(head) => head,
    // A branch nothing started on yet holds no work.
    Err(RunBaseError::Missing(_)) => return Err(DeliverRefusal::NothingToDeliver),
    Err(RunBaseError::Git(error)) => return Err(unreadable(&error)),
  };
  let into = match module.base() {
    BaseChoice::Branch(branch) => branch.clone(),
    BaseChoice::CheckedOut => checked_out_branch(&runner, git, root)
      .map_err(|error| unreadable(&error))?
      .ok_or(DeliverRefusal::NoBaseBranch)?,
  };
  if into == own.branch {
    return Err(DeliverRefusal::OnOwnBranch);
  }
  let base = match set_branch_tip(&runner, git, (root, &into)) {
    Ok(base) => base,
    Err(RunBaseError::Missing(_)) => return Err(DeliverRefusal::NoBaseBranch),
    Err(RunBaseError::Git(error)) => return Err(unreadable(&error)),
  };
  let delivery = ModuleDelivery {
    module: module.id(),
    branch: &own.branch,
    head: &head,
    into: &into,
    base: &base,
  };
  let found = module_merged_in(root, programs, &delivery).map_err(|error| unreadable(&error))?;
  let merge = match found {
    Some(merge) => merge,
    None if is_ancestor(&runner, git, (root, &head, &base)).map_err(|error| unreadable(&error))? => {
      return Err(DeliverRefusal::NothingToDeliver);
    }
    None => merge_module_in(root, programs, &delivery).map_err(refusal)?.revision,
  };
  let delivered = ModuleDelivered {
    module: module.id(),
    head,
    into,
    merge,
    at: Timestamp::now(),
  };
  db.keep_module_delivery(&delivered)?;
  Ok(delivered)
}

/// The keys of the tasks of module `module` whose latest run has not ended.
fn at_work(db: &ProjectDb, module: ModuleId) -> Result<Vec<String>, StoreError> {
  let mut working = Vec::new();
  for task in db.tasks()?.iter().filter(|task| task.module() == Some(module)) {
    let runs = db.runs_of_task(task.id()).map_err(run_store_error)?;
    if runs.last().is_some_and(|latest| match latest {
      Ok(run) => !run.status().is_terminal(),
      // A run that cannot be read may be working.
      Err(_) => true,
    }) {
      working.push(task.key().to_string());
    }
  }
  Ok(working)
}

fn refusal(error: ModuleMergeError) -> DeliverRefusal {
  match error {
    ModuleMergeError::Busy => DeliverRefusal::Busy,
    ModuleMergeError::Merge(MergeError::Conflict { files }) => DeliverRefusal::Conflict(files),
    ModuleMergeError::Merge(MergeError::UncommittedInTheWay { files }) => DeliverRefusal::InTheWay(files),
    ModuleMergeError::Store(error) => DeliverRefusal::Store(error),
    other => DeliverRefusal::Failed(other.to_string()),
  }
}

/// A module's own branch as its page shows it (`M02 · Module · Summary · its branch and Deliver` and `Parts ·
/// Delivery` on the board `Orchestrator autonomy · frames to approve`): where its work stands on the way to its base
/// branch, from the project's records alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryView {
  pub own: OwnBranch,
  /// The base branch the module names; `None`: the branch the checkout has checked out.
  pub into: Option<BranchName>,
  /// The module's own tasks, Canceled ones and its acceptances left out, and how many of them have merged work.
  pub tasks: usize,
  pub merged: usize,
  /// How many of them are neither Done nor Canceled.
  pub open: usize,
  /// The keys of the tasks whose latest run has not ended, acceptances too.
  pub at_work: Vec<String>,
  /// What the module waits for once its every task is done.
  pub step: DeliveryStep,
  /// Its latest delivery.
  pub delivered: Option<ModuleDelivered>,
  /// Its acceptance tasks, oldest first.
  pub acceptances: Vec<TaskId>,
}

impl DeliveryView {
  /// Module `module`'s own branch among `tasks`, the project's; `None` for a module without one. Blocks on SQLite.
  pub fn read(db: &ProjectDb, module: &Module, tasks: &[Task]) -> Result<Option<Self>, StoreError> {
    let Some(own) = module.own_branch().cloned() else {
      return Ok(None);
    };
    let acceptances: Vec<TaskId> = db
      .module_acceptances(module.id())?
      .into_iter()
      .map(|(task, _)| task)
      .collect();
    let (mut counted, mut merged, mut open) = (0, 0, 0);
    for task in tasks.iter().filter(|task| task.module() == Some(module.id())) {
      let status = task.organization().status();
      if acceptances.contains(&task.id()) || status == WorkflowStatus::Canceled {
        continue;
      }
      counted += 1;
      merged += usize::from(last_merge(db, task.id())?.is_some());
      open += usize::from(status != WorkflowStatus::Done);
    }
    Ok(Some(Self {
      own,
      into: match module.base() {
        BaseChoice::Branch(branch) => Some(branch.clone()),
        BaseChoice::CheckedOut => None,
      },
      tasks: counted,
      merged,
      open,
      at_work: at_work(db, module.id())?,
      step: facts(db, module)?.step(),
      delivered: db.module_delivery(module.id())?,
      acceptances,
    }))
  }

  /// Whether the branch holds merged work its base branch has not been given.
  pub const fn undelivered(&self) -> bool {
    self.merged > 0 && !matches!(self.step, DeliveryStep::Delivered)
  }
}
