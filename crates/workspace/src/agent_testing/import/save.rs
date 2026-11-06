//! Saving the person's chosen proposals: each becomes a new service or stack whose first draft is the proposal, so
//! nothing is published and no run starts it until a person publishes it. A proposal whose name the project has
//! already (or had) is kept as it is there, not replaced.

use std::{collections::BTreeSet, path::Path};

use orchester_core::{id::ResourceId, resource::ResourceKind, store::ResourceStore, time::Timestamp};
use orchester_store::ProjectDb;

use super::{Draft, Proposal, Proposals};
use crate::{
  agent_testing::data::create,
  messages::{Message, store_error},
};

/// The proposals the person keeps, by their names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choice {
  pub services: Vec<String>,
  pub stacks: Vec<String>,
}

/// A draft the import made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedDraft {
  pub kind: ResourceKind,
  pub name: String,
  pub id: ResourceId,
}

/// What saving did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Imported {
  pub saved: Vec<SavedDraft>,
  /// The chosen proposals of a name the project has already, of their kind, which were not saved.
  pub taken: Vec<(ResourceKind, String)>,
}

impl Proposals {
  /// The proposals `choice` names, each once, in the order they are proposed; `Err` names one that is not proposed.
  pub fn chosen(&self, choice: &Choice) -> Result<Vec<&Proposal>, Message> {
    let wanted = |kind: ResourceKind, names: &[String]| -> Result<BTreeSet<String>, Message> {
      for name in names {
        let proposed = self
          .proposals
          .iter()
          .any(|proposal| proposal.draft.kind() == kind && proposal.draft.name() == name);
        if !proposed {
          return Err(Message::new("services.import.errors.notProposed").with("name", name.clone()));
        }
      }
      Ok(names.iter().cloned().collect())
    };
    let services = wanted(ResourceKind::Service, &choice.services)?;
    let stacks = wanted(ResourceKind::Stack, &choice.stacks)?;
    Ok(
      self
        .proposals
        .iter()
        .filter(|proposal| match proposal.draft {
          Draft::Service(_) => services.contains(proposal.draft.name()),
          Draft::Stack(_) => stacks.contains(proposal.draft.name()),
        })
        .collect(),
    )
  }
}

/// Saves `chosen` as drafts of new services and stacks in the project at `root`, passing over a name the project has
/// already of its kind. Blocks on SQLite; a failure stops the saving there, and what was saved before stays.
pub fn save_in(root: &Path, chosen: &[&Proposal], now: Timestamp) -> Result<Imported, Message> {
  let mut db = ProjectDb::open(root).map_err(|error| store_error(&error))?;
  let mut imported = Imported::default();
  for proposal in chosen {
    let draft = &proposal.draft;
    let slug = draft.slug();
    let taken = db.slugs(draft.kind()).map_err(|error| store_error(&error))?;
    if taken.contains(&slug) {
      imported.taken.push((draft.kind(), draft.name().to_owned()));
      continue;
    }
    let id = match draft {
      Draft::Service(service) => create(&mut db, slug, service.as_ref(), now),
      Draft::Stack(stack) => create(&mut db, slug, stack, now),
    }
    .map_err(|error| store_error(&error))?;
    imported.saved.push(SavedDraft {
      kind: draft.kind(),
      name: draft.name().to_owned(),
      id,
    });
  }
  Ok(imported)
}
