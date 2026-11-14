//! A local module with a branch of its own, against a real git repository (`modules::delivery`,
//! `docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`): the module gets `module/<slug>` as its branch's name, its
//! delivery merges that branch into the module's base branch with a merge commit and is kept, a delivery with
//! nothing to deliver or one that conflicts changes nothing, and what the module waits for is read from its
//! acceptances and its delivery.
//! `cargo test -p orchester-workspace --test module_delivery module_delivery`.

use std::{fs, path::Path, process::Command};

use orchester_core::{
  id::{IdempotencyKey, ModuleId, ProjectId, ResourceId},
  module::{
    ModuleDetails, ModuleState,
    delivery::{Acceptance, DeliveryStep, PersonsDelivery},
  },
  name::Name,
  project::{Project, ProjectKey},
  store::TaskStore,
  task::{TaskDetails, TaskLimits, organization::ChangeActor},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::modules::delivery::{
  DeliverRefusal, DeliveryView, OwnChange, change_own_in, deliver_in, facts,
};

fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

/// A project in a git repository on `main`, with the module *Webhooks v2*.
struct Repo {
  dir: tempfile::TempDir,
  db: ProjectDb,
  module: ModuleId,
  programs: Programs,
}

impl Repo {
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("directory");
    let root = dir.path();
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Module delivery test"]);
    git(root, &["config", "user.email", "test@example.invalid"]);
    git(root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
    fs::write(root.join("shared.txt"), "base\n").expect("shared");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    let mut db = ProjectDb::create(root, project).expect("project");
    let details = ModuleDetails::new(
      Name::new("Webhooks v2").expect("name"),
      "",
      (None, None),
      ModuleState::Open,
    )
    .expect("details");
    let module = db.create_module(details, at(1)).expect("module").id();
    Self {
      dir,
      db,
      module,
      programs: Programs::processes(),
    }
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  /// Gives the module a branch of its own, and says what else `change` says.
  fn own(&self, change: &OwnChange) -> orchester_core::module::Module {
    let module = self.db.module(self.module).expect("read").expect("module");
    let change = OwnChange {
      own_branch: Some(true),
      ..change.clone()
    };
    change_own_in(self.root(), &self.programs, &module, &change).expect("its own branch")
  }

  /// Commits `text` as `file` on the module's branch, which is made at `main` when it is not there; the checkout
  /// comes back to `main`.
  fn work(&self, file: &str, text: &str) {
    let root = self.root();
    if git(root, &["branch", "--list", "module/webhooks-v2"]).is_empty() {
      git(root, &["branch", "module/webhooks-v2", "main"]);
    }
    git(root, &["switch", "--quiet", "module/webhooks-v2"]);
    fs::write(root.join(file), text).expect("work");
    git(root, &["add", "."]);
    git(root, &["commit", "--quiet", "-m", "module work"]);
    git(root, &["switch", "--quiet", "main"]);
  }

  fn deliver(&self) -> Result<orchester_core::module::delivery::ModuleDelivered, DeliverRefusal> {
    deliver_in(self.root(), &self.programs, self.module)
  }
}

#[test]
fn module_delivery_gives_a_module_a_branch_of_its_own_by_its_name() {
  let repo = Repo::new();
  let plain = repo.db.module(repo.module).expect("read").expect("module");
  assert!(plain.own_branch().is_none());
  // An acceptance pipeline is for a module with a branch of its own.
  let pipeline = ResourceId::generate();
  let refused = change_own_in(
    repo.root(),
    &repo.programs,
    &plain,
    &OwnChange {
      acceptance: Some(Some(pipeline)),
      ..OwnChange::default()
    },
  )
  .expect_err("refused");
  assert_eq!(refused.key, "modules.ownBranch.required");

  let owned = repo.own(&OwnChange {
    acceptance: Some(Some(pipeline)),
    deliver_when_accepted: Some(true),
    ..OwnChange::default()
  });
  let own = owned.own_branch().expect("its own branch");
  assert_eq!(own.branch.as_str(), "module/webhooks-v2");
  assert_eq!((own.acceptance, own.deliver_when_accepted), (Some(pipeline), true));
  assert_eq!(
    git(repo.root(), &["branch", "--list", "module/webhooks-v2"]),
    "",
    "the branch is made when the module's first task starts, not here"
  );
  // Kept, and the module's tasks' runs take it as their base.
  let stored = repo.db.module(repo.module).expect("read").expect("module");
  assert_eq!(stored.own_branch(), Some(own));
  assert_eq!(
    stored.run_base().branch().map(|branch| branch.as_str()),
    Some("module/webhooks-v2")
  );

  // A name the repository has already gets a number.
  git(repo.root(), &["branch", "module/billing", "main"]);
  let mut db = ProjectDb::open(repo.root()).expect("open");
  let details =
    ModuleDetails::new(Name::new("Billing").expect("name"), "", (None, None), ModuleState::Open).expect("details");
  let billing = db.create_module(details, at(2)).expect("module");
  let change = OwnChange {
    own_branch: Some(true),
    ..OwnChange::default()
  };
  let billing = change_own_in(repo.root(), &repo.programs, &billing, &change).expect("its own branch");
  assert_eq!(
    billing.own_branch().map(|own| own.branch.as_str()),
    Some("module/billing-2")
  );

  // Taking the branch away leaves it in the repository.
  let away = OwnChange {
    own_branch: Some(false),
    ..OwnChange::default()
  };
  let plain = change_own_in(repo.root(), &repo.programs, &billing, &away).expect("changed");
  assert!(plain.own_branch().is_none());
  assert_eq!(
    git(repo.root(), &["branch", "--list", "module/billing"]),
    "module/billing"
  );
}

#[test]
fn module_delivery_merges_the_modules_branch_into_its_base_and_keeps_it() {
  let repo = Repo::new();
  // A module without a branch of its own has nothing to deliver.
  assert_eq!(repo.deliver().expect_err("refused").code(), "no_own_branch");
  repo.own(&OwnChange::default());
  assert_eq!(
    repo.deliver().expect_err("refused").code(),
    "nothing_to_deliver",
    "nothing started on the branch yet"
  );

  repo.work("webhooks.txt", "retry three times\n");
  let head = git(repo.root(), &["rev-parse", "module/webhooks-v2"]);
  let delivered = repo.deliver().expect("delivered");
  assert_eq!(delivered.into.as_str(), "main");
  assert_eq!(delivered.head.as_str(), head);
  assert_eq!(git(repo.root(), &["rev-parse", "main"]), delivered.merge.as_str());
  assert_eq!(
    git(repo.root(), &["rev-list", "--parents", "-n", "1", "main"])
      .split(' ')
      .count(),
    3,
    "a merge commit"
  );
  assert_eq!(
    fs::read_to_string(repo.root().join("webhooks.txt")).expect("in the checkout"),
    "retry three times\n"
  );
  assert_eq!(
    git(repo.root(), &["symbolic-ref", "--short", "HEAD"]),
    "main",
    "the checkout stays on main"
  );
  let kept = repo.db.module_delivery(repo.module).expect("read").expect("kept");
  assert_eq!(kept, delivered);

  // Nothing more to deliver until more work is on the branch.
  assert_eq!(repo.deliver().expect_err("refused").code(), "nothing_to_deliver");
  repo.work("webhooks.txt", "retry five times\n");
  let again = repo.deliver().expect("delivered again");
  assert_ne!(again.merge, delivered.merge);
}

#[test]
fn module_delivery_that_conflicts_changes_nothing() {
  let repo = Repo::new();
  repo.own(&OwnChange::default());
  repo.work("shared.txt", "the module's line\n");
  fs::write(repo.root().join("shared.txt"), "the base's line\n").expect("edit");
  git(repo.root(), &["commit", "--quiet", "-am", "base moved"]);
  let main = git(repo.root(), &["rev-parse", "main"]);

  let refused = repo.deliver().expect_err("refused");
  assert!(
    matches!(&refused, DeliverRefusal::Conflict(files) if files == &["shared.txt".to_owned()]),
    "{refused:?}"
  );
  assert_eq!(refused.code(), "delivery_conflicts");
  assert_eq!(git(repo.root(), &["rev-parse", "main"]), main, "the base did not move");
  assert_eq!(
    git(repo.root(), &["status", "--porcelain"]),
    "",
    "the checkout is as it was"
  );
  assert!(repo.db.module_delivery(repo.module).expect("read").is_none());
}

#[test]
fn module_delivery_is_refused_while_a_task_of_the_module_is_still_to_accept() {
  let mut repo = Repo::new();
  let pipeline = ResourceId::generate();
  let module = repo.own(&OwnChange {
    acceptance: Some(Some(pipeline)),
    ..OwnChange::default()
  });
  // Nothing was merged into the branch: there is nothing to wait for.
  let read = facts(&repo.db, &module).expect("facts");
  assert_eq!((read.acceptance, read.delivered), (Acceptance::None, true));
  assert_eq!(read.step(), DeliveryStep::Delivered);

  // An acceptance task that has not run is under way; the module is accepted by none yet.
  let details = TaskDetails {
    title: Name::new("Accept the module Webhooks v2").expect("name"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  };
  let task = repo
    .db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(10))
    .expect("task")[0]
    .id();
  repo
    .db
    .set_task_module(task, Some(repo.module), ChangeActor::Orchester, at(11))
    .expect("in the module");
  repo.db.keep_module_acceptance(repo.module, task, at(12)).expect("kept");
  let read = facts(&repo.db, &module).expect("facts");
  assert_eq!((read.acceptances, read.acceptance), (1, Acceptance::UnderWay));

  // A module that names no acceptance pipeline is delivered by the person only.
  let unguarded = repo.own(&OwnChange {
    acceptance: Some(None),
    ..OwnChange::default()
  });
  let read = facts(&repo.db, &unguarded).expect("facts");
  assert!(!read.acceptance_pipeline);
  let undelivered = orchester_core::module::delivery::DeliveryFacts {
    delivered: false,
    acceptance: Acceptance::None,
    ..read
  };
  assert_eq!(undelivered.step(), DeliveryStep::Person(PersonsDelivery::NoAcceptance));
}

#[test]
fn module_delivery_view_says_where_a_branch_of_its_own_stands() {
  let repo = Repo::new();
  let module = repo.db.module(repo.module).expect("read").expect("module");
  assert_eq!(
    DeliveryView::read(&repo.db, &module, &[]).expect("read"),
    None,
    "a module without a branch of its own has no delivery to show"
  );

  let owned = repo.own(&OwnChange::default());
  let view = DeliveryView::read(&repo.db, &owned, &[])
    .expect("read")
    .expect("its own branch");
  assert_eq!(view.own.branch.as_str(), "module/webhooks-v2");
  assert_eq!(view.into, None, "the branch the checkout has checked out");
  assert_eq!((view.tasks, view.merged, view.open), (0, 0, 0));
  assert!(view.at_work.is_empty() && view.acceptances.is_empty());
  assert_eq!(view.step, DeliveryStep::Delivered, "nothing was merged into it");
  assert!(!view.undelivered());
  assert_eq!(view.delivered, None);

  repo.work(
    "webhooks.txt",
    "retry three times
",
  );
  let delivered = repo.deliver().expect("delivered");
  let view = DeliveryView::read(&repo.db, &owned, &[])
    .expect("read")
    .expect("its own branch");
  assert_eq!(view.delivered, Some(delivered), "its latest delivery");

  // With nothing merged that is not delivered, the branch may be taken away again.
  let off = OwnChange {
    own_branch: Some(false),
    ..OwnChange::default()
  };
  let plain = change_own_in(repo.root(), &repo.programs, &owned, &off).expect("turned off");
  assert_eq!(plain.own_branch(), None);
}
