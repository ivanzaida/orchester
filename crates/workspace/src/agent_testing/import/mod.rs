//! A first draft of a project's services and stacks, read once from its checkout (`docs/architecture/agent-testing.md`,
//! "Import"; the import page `DFWRJ`): `docker compose` files ([`compose`]), the scripts of `package.json` files
//! named like `dev`, `start` or `preview` ([`package`]), and Cargo binary targets ([`cargo`]). Every manifest the
//! listing finds is read, so the packages of npm, pnpm, yarn and Cargo workspaces are read as their own manifests.
//! Reading runs nothing ([`checkout`]). Each proposal names the file and key it came from and what of it Orchester
//! could not map; what is found but not proposed is said with why, and so is a file that could not be read. The
//! person's chosen proposals become drafts of new services and stacks ([`save`]); nothing is published.

mod cargo;
mod checkout;
mod compose;
mod local;
mod names;
mod package;
mod save;
mod yaml;

use std::path::Path;

use orchester_core::{
  resource::{ResourceKind, Slug},
  service::{Service, Stack},
  store::ResourceStore,
};
use orchester_store::ProjectDb;

pub use self::save::{Choice, Imported, SavedDraft, save_in};
use self::{
  checkout::{CARGO_FILE, COMPOSE_FILES, Checkout, PACKAGE_FILE},
  names::Names,
};
use super::data::{service_slug, stack_slug};
use crate::messages::{Message, store_error};

/// Where a proposal, or something passed over, came from: the checkout's file, with `/` between its parts, and the
/// key in it, e.g. `deploy/cloud/compose.yaml` and `services.api`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
  pub file: String,
  pub key: String,
}

/// A part of the source the proposal does not carry, by its key in the file (`services.api.volumes`), and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unmapped {
  pub key: String,
  pub reason: Message,
}

/// What a proposal would save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Draft {
  Service(Box<Service>),
  Stack(Stack),
}

impl Draft {
  pub fn name(&self) -> &str {
    match self {
      Self::Service(service) => service.name().as_str(),
      Self::Stack(stack) => stack.name().as_str(),
    }
  }

  pub const fn kind(&self) -> ResourceKind {
    match self {
      Self::Service(_) => ResourceKind::Service,
      Self::Stack(_) => ResourceKind::Stack,
    }
  }

  /// The slug it is stored under: its name.
  pub fn slug(&self) -> Slug {
    match self {
      Self::Service(service) => service_slug(service.name()),
      Self::Stack(stack) => stack_slug(stack.name()),
    }
  }
}

/// A service or stack the checkout suggests. Its name is its own among the proposals of its kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
  pub draft: Draft,
  pub source: Source,
  pub unmapped: Vec<Unmapped>,
  /// The project has a service or stack of its kind under its name already (or had one), which saving keeps.
  pub exists: bool,
}

/// Something the checkout describes that is not proposed, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
  pub source: Source,
  pub reason: Message,
}

/// A file or folder of the checkout that could not be read, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unreadable {
  pub file: String,
  pub reason: Message,
}

/// What the checkout suggests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Proposals {
  /// Compose files' services and stacks first, then `package.json` scripts, then Cargo binaries, each by file.
  pub proposals: Vec<Proposal>,
  pub skipped: Vec<Skipped>,
  pub unreadable: Vec<Unreadable>,
  /// The checkout has more entries than are listed; what lies past them is not read.
  pub truncated: bool,
}

/// What the checkout at `root` suggests, read from its files alone. Blocks on the file system.
pub fn read_checkout(root: &Path) -> Result<Proposals, Message> {
  let checkout = Checkout::open(root)?;
  let listing = checkout.list();
  let mut reading = Reading {
    unreadable: listing.unreadable,
    ..Reading::default()
  };
  let named = |names: &[&str]| -> Vec<String> {
    let file_name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_owned();
    listing
      .files
      .iter()
      .filter(|path| names.contains(&file_name(path).as_str()))
      .cloned()
      .collect()
  };
  for file in named(COMPOSE_FILES) {
    compose::read(&checkout, &file, &mut reading);
  }
  package::read(&checkout, &named(&[PACKAGE_FILE]), &mut reading);
  for file in named(&[CARGO_FILE]) {
    cargo::read(&checkout, &file, &mut reading);
  }
  Ok(Proposals {
    proposals: reading.proposals,
    skipped: reading.skipped,
    unreadable: reading.unreadable,
    truncated: listing.truncated,
  })
}

/// What the checkout of the project at `root` suggests, each proposal marked when the project has its name already.
/// Blocks on the file system and SQLite.
pub fn propose_in(root: &Path) -> Result<Proposals, Message> {
  let mut proposals = read_checkout(root)?;
  let db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  let services = db.slugs(ResourceKind::Service).map_err(|error| store_error(&error))?;
  let stacks = db.slugs(ResourceKind::Stack).map_err(|error| store_error(&error))?;
  for proposal in &mut proposals.proposals {
    let taken = match proposal.draft {
      Draft::Service(_) => &services,
      Draft::Stack(_) => &stacks,
    };
    proposal.exists = taken.contains(&proposal.draft.slug());
  }
  Ok(proposals)
}

/// What the readers found so far, with the names their proposals took.
#[derive(Default)]
struct Reading {
  names: Names,
  proposals: Vec<Proposal>,
  skipped: Vec<Skipped>,
  unreadable: Vec<Unreadable>,
}

impl Reading {
  fn propose(&mut self, draft: Draft, source: Source, unmapped: Vec<Unmapped>) {
    self.proposals.push(Proposal {
      draft,
      source,
      unmapped,
      exists: false,
    });
  }

  fn skip(&mut self, source: Source, reason: Message) {
    self.skipped.push(Skipped { source, reason });
  }

  fn unreadable(&mut self, file: &str, reason: Message) {
    self.unreadable.push(Unreadable {
      file: file.to_owned(),
      reason,
    });
  }
}

/// The folder of the checkout's file `path`; empty for the checkout itself.
fn folder_of(path: &str) -> &str {
  path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// `name` in the checkout's folder `folder`.
fn join(folder: &str, name: &str) -> String {
  if folder.is_empty() {
    name.to_owned()
  } else {
    format!("{folder}/{name}")
  }
}

/// What the source's `key` holds that Orchester has no field for.
fn no_field(key: String) -> Unmapped {
  Unmapped {
    key,
    reason: Message::new("services.import.unmapped.field"),
  }
}
