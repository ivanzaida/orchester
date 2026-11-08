//! One autosave of an admin's draft of a team MCP server ([`super::edit::Edit::autosave`]): its place in the
//! resource's lane, taken when it starts, and the blocking job that writes it. The job is on the pool from that moment,
//! not when the panel's future is first polled, so it still writes when the panel is closed first, and the text it
//! leaves to a newer save is always on the pool already. A save the store refused is told and tried again.

use std::{path::PathBuf, sync::Arc, time::Duration};

use lurq::app::ctx::StreamEmitter;
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::{
  agents::{
    save_lane::{SaveLane, Ticket},
    team_mcp_edit::{self, EditError, Saved},
  },
  app_mcp::setup::NewTeamMcpServer,
  messages,
  tasks::outside::{Outside, OutsideChanges},
};
use tokio::{runtime::Handle, task::JoinHandle};

/// How long a save the store refused waits before it is tried again: at once it would fail the same way.
const RETRY_AFTER: Duration = Duration::from_secs(3);

/// How a save ended: what was written, or nothing when a newer save started and writes what is typed now.
pub(super) type Answer = Result<Option<Saved>, EditError>;

/// A save, with its place in the lane.
pub(super) struct Save {
  lane: Arc<SaveLane>,
  runtime: Handle,
  changes: OutsideChanges,
  target: (PathBuf, ProjectId, ResourceId),
  /// The panel's last read held a draft.
  had_draft: bool,
  input: NewTeamMcpServer,
  ticket: Ticket,
}

impl Save {
  /// Takes a place in the lane and puts the write of `input` on the blocking pool.
  pub(super) fn start(
    (lane, runtime, changes): (Arc<SaveLane>, Handle, OutsideChanges),
    target: (PathBuf, ProjectId, ResourceId),
    (had_draft, input): (bool, NewTeamMcpServer),
  ) -> (Self, JoinHandle<Answer>) {
    let ticket = lane.start();
    let save = Self {
      lane,
      runtime,
      changes,
      target,
      had_draft,
      input,
      ticket,
    };
    let job = save.write();
    (save, job)
  }

  /// The write of this save, again with the place it already has: a retry that a newer save passed skips itself.
  fn write(&self) -> JoinHandle<Answer> {
    let (lane, changes, input) = (self.lane.clone(), self.changes.clone(), self.input.clone());
    let ticket = self.ticket;
    let ((root, project, id), had_draft) = (self.target.clone(), self.had_draft);
    self.runtime.spawn_blocking(move || {
      let saved = lane
        .run_if_newest(ticket, || team_mcp_edit::save(&root, id, &input))
        .transpose()?;
      if saved.is_some_and(|saved| matches!(saved, Saved::Draft(_)) != had_draft) {
        changes.record(project, Outside::Connections);
      }
      Ok(saved)
    })
  }

  /// Says how the write `job` ended, and tries again while the store refuses it.
  pub(super) async fn settle(self, mut job: JoinHandle<Answer>, emitter: &StreamEmitter<Option<Saved>, EditError>) {
    loop {
      match job.await {
        Ok(Err(EditError::Store(message))) => {
          emitter.reject(EditError::Store(message));
          tokio::time::sleep(RETRY_AFTER).await;
          job = self.write();
        }
        Ok(answer) => {
          match answer {
            Ok(saved) => emitter.emit(saved),
            Err(error) => emitter.reject(error),
          };
          return;
        }
        Err(error) => {
          tracing::error!(%error, "saving the draft did not finish");
          emitter.reject(EditError::Store(messages::interrupted()));
          return;
        }
      }
    }
  }
}
