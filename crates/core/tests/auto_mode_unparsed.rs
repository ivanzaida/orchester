//! ORC-339: auto mode reviews a command Orchester cannot read to the end instead of handing it to the person, told
//! which parts and why; what Orchester did read still holds (a credential, a write outside, the network while it is
//! off, a redirect in a read-only step, a destructive part). The classifier reads what agents run here (`git log -L`,
//! `-S`, `-G`, PowerShell's .NET path calls and `Get-*` reads), and git that stays in the run's own worktree and
//! branch is the run's, while git beyond them still holds.
//! `cargo test -p orchester-core --test auto_mode_unparsed auto_mode_unreadable`, `… classify_git_log_range`,
//! `… classify_pwsh_reads`, `… reset_hard_head`, `… git_in_run_worktree`.

use std::cell::RefCell;

use orchester_core::{
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  resource::ModelId,
  run::{
    ApprovalSubject, PendingToolApproval,
    review::{
      HoldRules, PushFacts, REVIEW_INSTRUCTIONS, RequestReviewer, Review, ReviewHold, ReviewInput, ReviewSpend,
      ReviewVerdict, Reviewed, ReviewedRequest, ReviewedStep, ReviewedTask, UnreadPart, UnreadWhy, review_hold,
      review_prompt, review_request,
    },
  },
  time::Timestamp,
};
use tempfile::TempDir;

/// A worktree with a source folder in a home folder, beside another repository.
struct Fixture {
  _root: TempDir,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("kontur");
    let temp = root.path().join("temp");
    for folder in [
      worktree.join("src"),
      worktree.join("crates/workspace/src/tasks"),
      home.join("other/.git"),
      home.join(".ssh"),
      temp.clone(),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("src/lib.rs"), "fn main() {}").expect("file");
    std::fs::write(worktree.join("regen.sh"), "echo hi").expect("script");
    std::fs::write(worktree.join("crates/workspace/src/tasks/needs_you.rs"), "x").expect("file");
    std::fs::write(home.join(".ssh/id_rsa"), "key").expect("key");
    let scope = CommandScope::new(&worktree, &[], Some(&home), Some(&temp));
    Self { _root: root, scope }
  }

  fn category(&self, command: &str) -> CommandCategory {
    ClassifiedCommand::classify(command, &self.scope).category()
  }

  fn command(&self, command: &str) -> ApprovalSubject {
    let approval = PendingToolApproval {
      id: "invocation:1".to_owned(),
      run: RunId::generate(),
      execution: StepExecutionId::generate(),
      invocation: InvocationId::generate(),
      agent: "Builder".to_owned(),
      command: true,
      scope: None,
      server: "command".to_owned(),
      action: command.to_owned(),
      target: None,
      requested_at: Timestamp::now(),
      native: None,
    };
    ApprovalSubject::of(&approval, &self.scope)
  }

  fn hold(&self, command: &str, permissions: &AgentPermissions) -> Option<ReviewHold> {
    review_hold(&self.command(command), &rules(permissions))
  }
}

fn step(repository: RepositoryAccess, network: NetworkAccess) -> AgentPermissions {
  AgentPermissions::new(repository, CommandPermission::AskBefore, network)
}

/// The Review step of ORC-243 run #147: reads the repository only, network off.
fn review_step() -> AgentPermissions {
  step(RepositoryAccess::ReadOnly, NetworkAccess::Off)
}

fn writing() -> AgentPermissions {
  step(RepositoryAccess::Write, NetworkAccess::On)
}

fn rules(permissions: &AgentPermissions) -> HoldRules<'_> {
  HoldRules {
    permissions: Some(permissions),
    push: PushFacts {
      branch: Some("orc/kon-1"),
      head: Some("orc/kon-1"),
      origin_kept: true,
    },
    ..HoldRules::default()
  }
}

fn input(command: &str, permissions: &AgentPermissions) -> ReviewInput {
  ReviewInput::new(
    ReviewedRequest {
      command: true,
      server: "command".to_owned(),
      action: command.to_owned(),
      arguments: None,
      note: None,
      working_folder: Some(".orchester/worktrees/w1".to_owned()),
    },
    ReviewedStep {
      name: "Review".to_owned(),
      repository: permissions.repository(),
      network: permissions.network().clone(),
      writable_folders: Vec::new(),
    },
    ReviewedTask {
      title: "Review the change".to_owned(),
      brief: String::new(),
      criteria: Vec::new(),
    },
    &[],
  )
  .expect("within size")
}

/// A reviewer that approves and keeps the parts it was told Orchester could not read.
struct Approves {
  told: RefCell<Option<Vec<UnreadPart>>>,
  model: ModelId,
}

impl Approves {
  fn new() -> Self {
    Self {
      told: RefCell::new(None),
      model: ModelId::new("claude-haiku-4-5-20251001").expect("model"),
    }
  }
}

impl RequestReviewer for Approves {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, input: &ReviewInput) -> Reviewed {
    *self.told.borrow_mut() = Some(input.unread_parts().to_vec());
    let verdict = ReviewVerdict::Approve {
      reason: "It only reads.".to_owned(),
    };
    Reviewed::new(Ok(verdict), ReviewSpend::Unknown)
  }
}

#[test]
fn auto_mode_unreadable_command_goes_to_the_reviewer_in_a_read_only_step_with_network_off() {
  let fixture = Fixture::new();
  let review = review_step();
  for (command, why) in [
    ("frobnicate --all src", UnreadWhy::Unknown),
    ("./regen.sh --check", UnreadWhy::Script),
    ("git status && frobnicate src", UnreadWhy::Unknown),
  ] {
    let subject = fixture.command(command);
    let reviewer = Approves::new();
    let reviewed = review_request(
      Some(&reviewer),
      (&subject, &rules(&review)),
      Ok(input(command, &review)),
    );
    assert!(reviewed.review.approves(), "{command}: {:?}", reviewed.review);
    let told = reviewer.told.borrow().clone().expect("reviewed");
    assert_eq!(told.len(), 1, "{command}");
    assert_eq!(told[0].why, why, "{command}");
  }
}

#[test]
fn auto_mode_unreadable_parts_reach_the_reviewer_with_why_and_the_steps_limits() {
  let fixture = Fixture::new();
  let review = review_step();
  let command = "git status; frobnicate --all";
  let subject = fixture.command(command);
  let unread = vec![UnreadPart {
    at: 2,
    why: UnreadWhy::Unknown,
  }];
  let prompt = review_prompt(&input(command, &review).with_unread(unread));
  assert!(
    prompt.contains(r#""command_parts_orchester_could_not_read":1"#),
    "{prompt}"
  );
  assert!(
    prompt.contains(r#""why_orchester_stopped_reading":[{"part":2,"why":"its program"#),
    "{prompt}"
  );
  assert!(prompt.contains(r#""network":"off: nothing may reach the network""#));
  assert!(prompt.contains("reads the repository only"));
  assert!(prompt.contains(r#""working_folder":".orchester/worktrees/w1""#));
  assert_eq!(review_hold(&subject, &rules(&review)), None);
  // The reviewer is told how to judge such a command in a confined step, and what it must still ask about.
  for words in [
    "In a step that reads the repository only or whose network is off, approve such a command when its words show \
     it only reads",
    "ask when it may write in a read-only step, reach the network while it is off, or run a script whose content \
     you do not see",
    "read a credential or secret, install, delete outside the worktree or rewrite or discard history",
    "that alone is never a reason to ask",
  ] {
    assert!(REVIEW_INSTRUCTIONS.contains(words), "{words}");
  }
}

#[test]
fn auto_mode_unreadable_command_still_holds_for_what_orchester_did_read() {
  let fixture = Fixture::new();
  let review = review_step();
  let offline = step(RepositoryAccess::Write, NetworkAccess::Off);
  let writing = writing();
  for (command, permissions, hold) in [
    ("frobnicate --all > listing.txt", &review, ReviewHold::ReadOnlyStep),
    ("./regen.sh > out.txt", &review, ReviewHold::ReadOnlyStep),
    ("frobnicate; cat ~/.ssh/id_rsa", &writing, ReviewHold::Secrets),
    ("frobnicate > ../../out.txt", &writing, ReviewHold::OutsideFolders),
    ("frobnicate && rm -rf ~/Documents", &writing, ReviewHold::Destructive),
    ("frobnicate && git push --force origin main", &writing, ReviewHold::Push),
    (
      "frobnicate && curl https://example.dev",
      &offline,
      ReviewHold::NetworkOff,
    ),
    ("frobnicate && npm install left-pad", &offline, ReviewHold::NetworkOff),
    (
      "frobnicate && cp src/lib.rs ../../lib.rs",
      &writing,
      ReviewHold::OutsideFolders,
    ),
  ] {
    assert_eq!(fixture.hold(command, permissions), Some(hold), "{command}");
  }
  let subject = fixture.command("frobnicate; cat ~/.ssh/id_rsa");
  let reviewer = Approves::new();
  let reviewed = review_request(Some(&reviewer), (&subject, &rules(&writing)), Ok(input("x", &writing)));
  assert_eq!(
    reviewed.review,
    Review::Held {
      hold: ReviewHold::Secrets
    }
  );
  assert!(reviewer.told.borrow().is_none(), "no review");
}

/// ORC-243 run #147's Review step: the request the person had to answer.
#[test]
fn classify_pwsh_reads_of_net_paths_are_read_only_and_reviewed_or_run() {
  let fixture = Fixture::new();
  let review = review_step();
  for command in [
    r#"powershell -NoProfile -Command "[System.IO.Path]::GetFullPath('C:\wt\kontur\src'); [System.IO.Path]::GetFullPath('src')""#,
    r#"pwsh -NoProfile -Command "[IO.Path]::Combine('src', 'lib.rs')""#,
    r#"pwsh -NoProfile -Command "[System.IO.Path]::GetFileName('src/lib.rs'); Get-Content src/lib.rs""#,
    r#"pwsh -NoProfile -Command "[System.IO.File]::Exists('src/lib.rs'); Test-Path src""#,
    r#"pwsh -NoProfile -Command "Select-String -Path src/lib.rs -Pattern main""#,
    r#"pwsh -NoProfile -Command "Get-ChildItem src -Recurse -Filter *.rs""#,
  ] {
    assert_eq!(fixture.category(command), CommandCategory::ReadOnly, "{command}");
    assert_eq!(fixture.hold(command, &review), None, "{command}");
  }
}

#[test]
fn classify_pwsh_reads_leave_what_reaches_further_unread_or_held() {
  let fixture = Fixture::new();
  for command in [
    // A call of a method that writes, a variable, a pipe into a writer, a profile.
    r#"pwsh -NoProfile -Command "[System.IO.File]::WriteAllText('src/a.txt', 'x')""#,
    r#"pwsh -NoProfile -Command "[System.IO.Path]::GetFullPath($env:USERPROFILE)""#,
    r#"pwsh -NoProfile -Command "[System.IO.Path]::GetFullPath('src') | Out-File x.txt""#,
    r#"pwsh -NoProfile -Command "[System.Diagnostics.Process]::Start('calc')""#,
    r#"pwsh -NoProfile -Command "[System.IO.Path]::GetFullPath('src'); Remove-Item src -Recurse""#,
  ] {
    assert_ne!(fixture.category(command), CommandCategory::ReadOnly, "{command}");
  }
  assert_eq!(
    fixture.category(r#"pwsh -Command "[System.IO.Path]::GetFullPath('src')""#),
    CommandCategory::Unknown,
    "without -NoProfile the user's profile runs first: not read"
  );
  let secret = r#"pwsh -NoProfile -Command "[System.IO.File]::ReadAllText('../.ssh/id_rsa')""#;
  assert!(ClassifiedCommand::classify(secret, &fixture.scope).reads_secrets());
  assert_eq!(fixture.hold(secret, &writing()), Some(ReviewHold::Secrets));
}

/// ORC-150 run #145's Implement step: `git log -L` was not read to the end.
#[test]
fn classify_git_log_range_and_pickaxe_are_read_only() {
  let fixture = Fixture::new();
  for command in [
    "git log -L329,331:crates/workspace/src/tasks/needs_you.rs --oneline -n 6",
    "git log -L 329,331:crates/workspace/src/tasks/needs_you.rs",
    "git log -L:needs_you:crates/workspace/src/tasks/needs_you.rs -n 2",
    "git log -Sneeds_you --oneline",
    "git log -S needs_you -- crates/workspace",
    "git log -G'fn needs_you' --oneline -n 3",
  ] {
    assert_eq!(fixture.category(command), CommandCategory::ReadOnly, "{command}");
    assert_eq!(fixture.hold(command, &review_step()), None, "{command}");
  }
  for command in ["git log -L329,331", "git log -L1,2:src/lib.rs --output=x.txt"] {
    assert_eq!(fixture.category(command), CommandCategory::Unknown, "{command}");
  }
  let secret = "git log -L1,2:../.ssh/id_rsa";
  assert!(
    ClassifiedCommand::classify(secret, &fixture.scope).reads_secrets(),
    "the file is placed"
  );
}

/// ORC-242 run #144 waited on exactly this.
#[test]
fn reset_hard_head_in_the_runs_worktree_is_reviewable() {
  let fixture = Fixture::new();
  let writing = writing();
  for command in [
    "git reset --hard HEAD",
    "git reset --hard",
    "git reset --hard HEAD~2",
    "git reset --hard @^",
    "git reset --hard 4f2a9c1",
    "git reset --keep HEAD",
  ] {
    assert_eq!(fixture.category(command), CommandCategory::Worktree, "{command}");
    assert_eq!(fixture.hold(command, &writing), None, "{command}");
  }
  for command in ["git reset --hard origin/main", "git reset --hard main"] {
    assert_eq!(
      fixture.category(command),
      CommandCategory::HistoryDestructive,
      "{command}"
    );
    assert_eq!(
      fixture.hold(command, &writing),
      Some(ReviewHold::Destructive),
      "{command}"
    );
  }
  // In another repository, or one nested in the worktree, it rewrites what is not the run's.
  for command in [
    "git -C ../other reset --hard HEAD",
    "cd ../other && git reset --hard HEAD",
    "git -C src reset --hard HEAD",
  ] {
    assert_eq!(
      fixture.hold(command, &writing),
      Some(ReviewHold::Destructive),
      "{command}"
    );
  }
  assert_eq!(
    fixture.hold("git reset --hard HEAD", &review_step()),
    Some(ReviewHold::ReadOnlyStep),
    "a read-only step still writes nothing"
  );
}

#[test]
fn git_in_run_worktree_reads_and_changes_reach_no_person() {
  let fixture = Fixture::new();
  let writing = writing();
  for command in [
    "git status",
    "git diff HEAD~1",
    "git log --oneline -n 5",
    "git show HEAD",
    "git blame src/lib.rs",
    "git reflog",
    "git reflog -n 5",
  ] {
    assert_eq!(fixture.category(command), CommandCategory::ReadOnly, "{command}");
    assert_eq!(fixture.hold(command, &writing), None, "{command}");
  }
  for command in [
    "git add src/lib.rs",
    "git rm --cached src/lib.rs",
    "git mv src/lib.rs src/main.rs",
    "git restore src/lib.rs",
    "git checkout -- src/lib.rs",
    "git commit -m wip",
    "git commit --amend --no-edit",
    "git stash push -m wip",
    "git stash pop",
    "git stash apply",
    "git stash drop",
    "git reset --soft HEAD~1",
    "git reset --mixed HEAD",
    "git revert --no-edit HEAD",
    "git cherry-pick 4f2a9c1",
    "git merge --no-ff master",
    "git rebase master",
    "git rebase --onto master HEAD~3",
    "git rebase --continue",
    "git rebase --abort",
    "git clean -fd",
  ] {
    assert_eq!(fixture.category(command), CommandCategory::Worktree, "{command}");
    assert_eq!(fixture.hold(command, &writing), None, "{command}");
  }
}

#[test]
fn git_in_run_worktree_still_holds_what_reaches_beyond_it() {
  let fixture = Fixture::new();
  let writing = writing();
  for (command, hold) in [
    ("git push origin main", ReviewHold::Push),
    ("git push --force origin orc/kon-1", ReviewHold::Push),
    ("git branch -D old", ReviewHold::Destructive),
    ("git branch other", ReviewHold::Destructive),
    ("git branch -f master HEAD", ReviewHold::Destructive),
    ("git tag v1.0", ReviewHold::Destructive),
    ("git tag -d v1.0", ReviewHold::Destructive),
    ("git update-ref refs/heads/main HEAD", ReviewHold::Destructive),
    ("git rebase master other", ReviewHold::Destructive),
    ("git rebase --update-refs master", ReviewHold::Destructive),
    ("git worktree add ../w2", ReviewHold::Destructive),
    ("git worktree remove ../w2", ReviewHold::Destructive),
    ("git worktree prune", ReviewHold::Destructive),
    ("git config user.name Bot", ReviewHold::Destructive),
    ("git config --global core.editor vim", ReviewHold::Destructive),
    ("git config --unset core.hooksPath", ReviewHold::Destructive),
    ("git gc --prune=now", ReviewHold::Destructive),
    ("git prune", ReviewHold::Destructive),
    ("git reflog expire --expire=now --all", ReviewHold::Destructive),
    ("git filter-branch --tree-filter true HEAD", ReviewHold::Destructive),
    ("git filter-repo --path src", ReviewHold::Destructive),
    ("git stash clear", ReviewHold::Destructive),
    ("git -C ../other status", ReviewHold::OutsideFolders),
    ("git -C ../other commit -m x", ReviewHold::OutsideFolders),
    ("git --git-dir=../other/.git log", ReviewHold::OutsideFolders),
    ("git --git-dir ../other/.git log", ReviewHold::OutsideFolders),
    ("git --work-tree=../other status", ReviewHold::OutsideFolders),
    ("cd ../other && git status", ReviewHold::OutsideFolders),
    ("cd ../other && git reset --hard HEAD", ReviewHold::Destructive),
  ] {
    assert_eq!(fixture.hold(command, &writing), Some(hold), "{command}");
  }
  // A worktree-only setting changes only the run's worktree; what it names to start, the reviewer judges.
  assert_eq!(
    fixture.category("git config --worktree core.sparseCheckout true"),
    CommandCategory::Unknown
  );
  assert_eq!(
    fixture.hold("git config --worktree core.sparseCheckout true", &writing),
    None
  );
}
