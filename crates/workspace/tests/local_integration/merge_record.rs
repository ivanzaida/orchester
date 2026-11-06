//! *Record merge* on a run's attempt page (ORC-363, 09g `i2rVv`): the merge Git made and Orchester could not record is
//! recorded as Git made it, once its base branch holds it, and never merged again. `cargo test -p orchester-workspace
//! --test local_integration merge_record`.

use orchester_core::{
  integration::{
    MergeKind,
    attempt::{AttemptOutcome, MergeAsker, MergeAttempt, MergeVia},
  },
  revision::Revision,
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{integration::IntegrationProblem, merge_record::record_merge};

use super::support::{Fixture, git};

#[test]
fn merge_record_records_the_merge_git_made_once() {
  let fixture = Fixture::new(false);
  let programs = Programs::processes();
  let refused = record_merge(&fixture.worktrees(), &programs, fixture.run).expect_err("no merge Git made yet");
  assert!(
    matches!(&refused, IntegrationProblem::Store(message) if message.key == "runs.merge.failure.nothingToRecord"),
    "{refused:?}"
  );
  let mut db = ProjectDb::open(fixture.root()).expect("db");
  let run = db.run(fixture.run).expect("read").expect("the run");
  git(
    fixture.root(),
    &["merge", "--no-ff", "--no-edit", run.branch().as_str()],
  );
  let head = git(fixture.root(), &["rev-parse", "HEAD"]);
  let revision = Revision::new(head.trim()).expect("a commit");
  let attempt = MergeAttempt::new(
    (fixture.run, Timestamp::now()),
    (MergeAsker::Person, MergeVia::TaskPage),
    AttemptOutcome::NotRecorded {
      revision: revision.clone(),
    },
    Some("the project database stayed busy"),
  );
  db.record_merge_attempt(&attempt).expect("the attempt");

  let merge = record_merge(&fixture.worktrees(), &programs, fixture.run).expect("recorded");
  assert_eq!(merge.revision, revision);
  assert_eq!(merge.kind, MergeKind::MergeCommit);
  assert_eq!(Some(&merge.into), run.base_branch());
  assert_eq!(db.run_merge(fixture.run).expect("read"), Some(merge));
  assert_eq!(
    git(fixture.root(), &["rev-parse", "HEAD"]).trim(),
    revision.as_str(),
    "nothing was merged again"
  );
  assert_eq!(
    record_merge(&fixture.worktrees(), &programs, fixture.run).expect_err("recorded already"),
    IntegrationProblem::AlreadyMerged
  );
}
