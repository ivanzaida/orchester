//! Docs (`docs/design/docs.md`): a project's documents, kept by Orchester rather than in the repository. A doc
//! sits in a tree of folders; its text is Markdown. Edits are kept as a draft per doc and author, and a commit makes
//! the draft a new immutable version, numbered from 1. A commit names the version its draft was based on, and a newer
//! committed version refuses it ([`can_commit`], *Conflicts*). Archiving hides a doc and keeps every version.
//!
//! Rules that need every row of a project (a slug unique within its folder, one draft per doc and author) are the
//! store's; this module holds the rules themselves.

mod doc;
mod draft;
mod folder;
mod title;
mod version;

pub use self::{
  doc::{Doc, DocError},
  draft::{CommitRefusal, DocDraft, can_commit},
  folder::{DocFolder, FolderError, check_folder_move},
  title::{DocSlug, DocSlugError, DocTitle, DocTitleError},
  version::{
    Commit, CommitMessage, CommitMessageError, DocVersion, DocVersionNumber, StoredDocVersion, VersionNumberError,
  },
};
use crate::memory::MemoryAuthor;

/// Who wrote a draft or committed a version: a person, or an agent with its run and step. The same author a memory
/// record names.
pub type DocAuthor = MemoryAuthor;
