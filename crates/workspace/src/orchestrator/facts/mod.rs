//! What a card of the Orchestrator chat shows of what an action publishes, cancels, merges or started
//! (`Orchestrator · publish proposal` `b55NqO`, its states `UKgrR`, `Orchestrator · run proposals · states` `Ee2JE`):
//! the draft and its versions, where it applies, the runs under way that keep the version before, what it widens,
//! what its kind changes and its text's diff; or the run, where it stands, its branch and what it changed. They are
//! read when the card shows, so they say how things stand now, beside what the action kept.

mod publication;
mod run;
mod text;

use std::path::Path;

use orchester_core::{
  id::{ResourceId, RunId},
  orchestrator::{
    ChatAction,
    change::{Change, Publishing},
  },
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;

pub use self::text::{LineChange, TextDiff, TextLine};
use crate::messages::{self, Message};

/// What a card shows beside what its action kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Facts {
  Publication(Publication),
  Run(RunFacts),
}

/// A draft's publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publication {
  pub of: Publishing,
  pub resource: ResourceId,
  /// Its name as people read it now (a profile's role).
  pub name: String,
  /// The version it publishes.
  pub version: u32,
  /// When the version before it was published, when there is one.
  pub before_published: Option<Timestamp>,
  /// When the draft it publishes was saved.
  pub saved: Timestamp,
  /// Where an instruction or skill applies.
  pub places: Vec<Place>,
  /// Runs under way that keep the version before it.
  pub keeping: usize,
  /// What publishing the draft widens in what agents may do, while it is still a draft.
  pub widens: Vec<Message>,
  /// What a pipeline, profile, skill or tool is, as its kind tells it.
  pub detail: Option<Detail>,
  /// The text an instruction or skill changes, while the draft is the one proposed.
  pub diff: Option<TextDiff>,
}

/// Where an instruction or skill applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
  Project,
  /// The agents of a role.
  Role(String),
  Pipeline {
    pipeline: ResourceId,
    name: String,
  },
  Step {
    pipeline: ResourceId,
    name: String,
    step: String,
  },
}

/// What a publication's kind tells of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Detail {
  /// A pipeline's steps, and how many the draft adds, removes or changes.
  Steps { count: usize, changed: usize },
  /// A profile's connection and model.
  Model { connection: String, model: String },
  /// A skill's files.
  Files { count: usize },
  /// A command tool's program and arguments.
  Command { program: String, args: Vec<String> },
}

/// A run a cancel, merge or start acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunFacts {
  pub run: RunId,
  pub number: u32,
  /// Its pipeline's name.
  pub pipeline: String,
  /// Its task's key and title.
  pub task: Option<(String, String)>,
  pub stands: Stands,
  /// Its branch (`None` for a run that never started, which made none), and the branch it merges into.
  pub branch: Option<String>,
  pub into: Option<String>,
  /// What its last checkpoint changed over its base: files, lines added and removed.
  pub changes: Option<(usize, u64, u64)>,
}

/// Where a run stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stands {
  /// It works or waits: the step it is on (its place and name), and its tokens.
  Going { step: Option<(usize, String)>, tokens: u64 },
  /// It was cancelled: the step it stopped at, and its tokens.
  Cancelled { step: Option<(usize, String)>, tokens: u64 },
  /// It succeeded, and when; not merged.
  Ready { since: Option<Timestamp> },
  /// It was merged at `at`, into a commit people read by its first characters.
  Merged { at: Timestamp, revision: String },
  /// It failed.
  Failed,
}

/// What a card of `action` in the project at `root` shows of it; `None` for an action about nothing more than its
/// task. `programs` read a merge's changes. Blocks on SQLite and git.
pub fn load_in(root: &Path, programs: &Programs, action: &ChatAction) -> Result<Option<Facts>, Message> {
  let db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let facts = match (&action.change, action.started) {
    (Some(change), _) => match &change.change {
      Change::Publish {
        of,
        resource,
        version,
        saved,
      } => publication::read(&db, (*of, *resource), (*version, *saved)).map(|found| found.map(Facts::Publication)),
      Change::Cancel { run, .. }
      | Change::Merge { run, .. }
      | Change::Answer { run, .. }
      | Change::Message { run, .. } => run::read(root, &db, programs, *run).map(|found| found.map(Facts::Run)),
    },
    (None, Some(run)) => run::read(root, &db, programs, run).map(|found| found.map(Facts::Run)),
    (None, None) => Ok(None),
  };
  facts.map_err(|error| messages::store_error(&error))
}
