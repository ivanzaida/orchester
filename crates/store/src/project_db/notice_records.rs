//! What the runner records of a run's notice that its base branch moved (ORC-54, ORC-151), as values that can be sent
//! (`docs/architecture/remote-runners.md`, rule 2): the files it overlaps in, its delivery to an agent, and the
//! runner's own merge of it. Each applies only while the notice is still pending as it was read. Each record holds the
//! columns its write binds, and [`ProjectDb::write_notice`] writes it with the statement the direct writes
//! (`note_base_notice_overlap`, `deliver_base_notice`, `merge_base_notice`) use, as they go through it too.

use orchester_core::{
  base_move::BaseNotice,
  id::{InvocationId, NoticeId, StepExecutionId},
  revision::Revision,
  time::Timestamp,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::{ProjectDb, base_notices::overlap_body};
use crate::StoreError;

/// A change to a run's pending base notice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeRecord {
  /// The notice, as it was read.
  notice: NoticeId,
  revision: Revision,
  change: NoticeChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum NoticeChange {
  /// Its body, with the files it overlaps in.
  Overlap { body: String },
  Delivered {
    execution: StepExecutionId,
    invocation: InvocationId,
    at: Timestamp,
  },
  Merged {
    execution: StepExecutionId,
    merged: Revision,
    at: Timestamp,
  },
}

impl NoticeRecord {
  fn of(notice: &BaseNotice, change: NoticeChange) -> Self {
    Self {
      notice: notice.id,
      revision: notice.revision.clone(),
      change,
    }
  }

  /// Pending `notice` overlaps the run's changes also in `files`.
  pub fn overlap(notice: &BaseNotice, files: &[String]) -> Result<Self, StoreError> {
    Ok(Self::of(
      notice,
      NoticeChange::Overlap {
        body: overlap_body(notice, files)?,
      },
    ))
  }

  /// `notice` went to invocation `invocation` of `execution`, which started with it at `at`.
  pub fn delivered(
    notice: &BaseNotice,
    (execution, invocation): (StepExecutionId, InvocationId),
    at: Timestamp,
  ) -> Self {
    Self::of(
      notice,
      NoticeChange::Delivered {
        execution,
        invocation,
        at,
      },
    )
  }

  /// The runner merged `notice` itself at `at`, with the merge commit `merged`, as `execution` started.
  pub fn merged(notice: &BaseNotice, (execution, merged): (StepExecutionId, &Revision), at: Timestamp) -> Self {
    Self::of(
      notice,
      NoticeChange::Merged {
        execution,
        merged: merged.clone(),
        at,
      },
    )
  }
}

impl ProjectDb {
  /// Applies `record` while its notice is still pending as it was read: `false`, with nothing stored, once a newer
  /// move replaced it or it was delivered already.
  pub fn write_notice(&mut self, record: &NoticeRecord) -> Result<bool, StoreError> {
    let (id, revision) = (record.notice.to_string(), record.revision.as_str());
    let changed = match &record.change {
      NoticeChange::Overlap { body } => self.connection.execute(
        "UPDATE base_notices SET body = ?1 WHERE id = ?2 AND revision = ?3 AND delivered_at IS NULL",
        params![body, id, revision],
      )?,
      NoticeChange::Delivered {
        execution,
        invocation,
        at,
      } => self.connection.execute(
        "UPDATE base_notices SET execution_id = ?1, invocation_id = ?2, delivered_at = ?3 \
         WHERE id = ?4 AND revision = ?5 AND delivered_at IS NULL",
        params![
          execution.to_string(),
          invocation.to_string(),
          at.unix_millis(),
          id,
          revision,
        ],
      )?,
      NoticeChange::Merged { execution, merged, at } => self.connection.execute(
        "UPDATE base_notices SET execution_id = ?1, merged_revision = ?2, delivered_at = ?3 \
         WHERE id = ?4 AND revision = ?5 AND delivered_at IS NULL",
        params![execution.to_string(), merged.as_str(), at.unix_millis(), id, revision],
      )?,
    };
    Ok(changed == 1)
  }
}
