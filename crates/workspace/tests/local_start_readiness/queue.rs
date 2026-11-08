//! The board's move to Todo queues a draft, and the runner starts a queued task in its turn without asking again
//! whether its pipeline is ready here (ORC-840): a draft that is not ready stays a draft with the short reason, and a
//! ready one queues.

use orchester_core::task::{TaskStatus, organization::ChangeActor};
use orchester_workspace::secrets::Vault;

use super::not_ready;

#[test]
fn local_start_the_board_queues_a_ready_draft_and_keeps_one_that_is_not_ready_a_draft() {
  let (kontur, program, _outside) = not_ready();
  let before = (TaskStatus::Draft, kontur.task().revision().get(), 0);
  let task = kontur.task().id();
  let (_, servers, _) = &kontur.follow;
  let vault = Vault::keychain();
  let queue = || {
    kontur.runtime.block_on(kontur.workspace.queue_draft(
      kontur.dir.path(),
      (&vault, servers),
      task,
      ChangeActor::Person,
    ))
  };

  let refused = queue().expect_err("the pipeline is not ready here");
  assert_eq!(refused.key, "notReadyHere.refusedShort");
  assert_eq!(refused.args, [("pipeline", "Quick fix".to_owned())]);
  assert_eq!(
    kontur.standing(),
    before,
    "the task stays a draft: no status, no revision, no run"
  );

  std::fs::write(&program, "#!/bin/sh\n").expect("the program appears");
  queue().expect("ready, the draft queues");
  assert_eq!(kontur.standing().0, TaskStatus::Queued);
}
