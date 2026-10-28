//! A draft of a doc and the rule that decides whether it may be committed.

use thiserror::Error;

use super::{DocAuthor, DocVersion, DocVersionNumber};
use crate::{id::DocId, time::Timestamp};

/// Why a draft is not committed.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum CommitRefusal {
  /// Someone committed since the draft was started: the person merges the two before committing again
  /// (`docs/design/docs.md`, *Conflicts*).
  #[error("the draft is based on an older version than {latest}, which was committed since")]
  Outdated {
    /// `None` for a draft of a doc that had no version when it was started.
    based_on: Option<DocVersionNumber>,
    latest: DocVersionNumber,
  },
  /// The draft names a version the doc does not have.
  #[error("the draft is based on {based_on}, which the doc does not have")]
  UnknownBase { based_on: DocVersionNumber },
  #[error("the draft is of doc {draft}, the latest version of doc {latest}")]
  OtherDoc { draft: DocId, latest: DocId },
  #[error("doc {0} has as many versions as can be numbered")]
  NoNumberLeft(DocId),
}

/// One author's uncommitted change to a doc: what they are writing, and the version it started from. Nobody else
/// reads it until it is committed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocDraft {
  doc: DocId,
  author: DocAuthor,
  body: String,
  based_on: Option<DocVersionNumber>,
  updated_at: Timestamp,
}

impl DocDraft {
  /// `author`'s draft of `doc`, started from version `based_on` (`None`: the doc was never committed).
  pub const fn new(
    doc: DocId,
    author: DocAuthor,
    body: String,
    based_on: Option<DocVersionNumber>,
    updated_at: Timestamp,
  ) -> Self {
    Self {
      doc,
      author,
      body,
      based_on,
      updated_at,
    }
  }

  /// The author writes on: the draft keeps the version it was based on.
  pub fn edit(&mut self, body: String, now: Timestamp) {
    self.body = body;
    self.updated_at = now;
  }

  pub const fn doc(&self) -> DocId {
    self.doc
  }

  /// Whose draft it is.
  pub const fn author(&self) -> &DocAuthor {
    &self.author
  }

  pub fn body(&self) -> &str {
    &self.body
  }

  pub const fn based_on(&self) -> Option<DocVersionNumber> {
    self.based_on
  }

  pub const fn updated_at(&self) -> Timestamp {
    self.updated_at
  }
}

/// Whether `draft` may be committed while `latest` is its doc's latest version (`None`: never committed), and the
/// number the new version gets. Only a draft based on the latest version is committed.
pub fn can_commit(draft: &DocDraft, latest: Option<&DocVersion>) -> Result<DocVersionNumber, CommitRefusal> {
  let Some(latest) = latest else {
    return match draft.based_on {
      None => Ok(DocVersionNumber::FIRST),
      Some(based_on) => Err(CommitRefusal::UnknownBase { based_on }),
    };
  };
  if latest.doc() != draft.doc {
    return Err(CommitRefusal::OtherDoc {
      draft: draft.doc,
      latest: latest.doc(),
    });
  }
  match draft.based_on {
    Some(based_on) if based_on == latest.number() => {
      latest.number().next().ok_or(CommitRefusal::NoNumberLeft(draft.doc))
    }
    Some(based_on) if based_on > latest.number() => Err(CommitRefusal::UnknownBase { based_on }),
    based_on => Err(CommitRefusal::Outdated {
      based_on,
      latest: latest.number(),
    }),
  }
}
