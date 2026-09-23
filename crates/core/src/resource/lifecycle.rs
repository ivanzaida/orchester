//! Versioned configuration: agent profiles, instructions, pipelines and tools share one
//! lifecycle — one editable draft and immutable published versions `v1..vN`. Runs pin
//! versions, so publishing affects new runs only.

use std::{fmt, num::NonZeroU32};

use thiserror::Error;

use crate::{
  id::{ResourceId, VersionId},
  name::Name,
  time::Timestamp,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceKind {
  AgentProfile,
  Instruction,
  Pipeline,
  Tool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlugError {
  #[error(
    "slug must be 1–{max} characters of lowercase letters, digits and single hyphens, got `{0}`",
    max = Slug::MAX_CHARS
  )]
  Invalid(String),
}

/// Stable, URL-safe handle of a resource within its project, e.g. `feature-delivery`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
  pub const MAX_CHARS: usize = 64;

  pub fn new(value: &str) -> Result<Self, SlugError> {
    let allowed = value
      .chars()
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let hyphens_inside = !value.starts_with('-') && !value.ends_with('-') && !value.contains("--");
    if !value.is_empty() && value.len() <= Self::MAX_CHARS && allowed && hyphens_inside {
      Ok(Self(value.to_owned()))
    } else {
      Err(SlugError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// Published version number, starting at 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionNumber(NonZeroU32);

impl VersionNumber {
  pub const FIRST: Self = Self(NonZeroU32::MIN);

  pub const fn new(number: NonZeroU32) -> Self {
    Self(number)
  }

  pub const fn get(self) -> u32 {
    self.0.get()
  }

  fn next(self) -> Option<Self> {
    self.0.checked_add(1).map(Self)
  }
}

impl fmt::Display for VersionNumber {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "v{}", self.0)
  }
}

/// Identity and metadata of a resource; its content lives in drafts and versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
  id: ResourceId,
  kind: ResourceKind,
  slug: Slug,
  name: Name,
  created_at: Timestamp,
}

impl Resource {
  pub const fn new(id: ResourceId, kind: ResourceKind, slug: Slug, name: Name, created_at: Timestamp) -> Self {
    Self {
      id,
      kind,
      slug,
      name,
      created_at,
    }
  }

  pub const fn id(&self) -> ResourceId {
    self.id
  }

  pub const fn kind(&self) -> ResourceKind {
    self.kind
  }

  pub fn slug(&self) -> &Slug {
    &self.slug
  }

  pub fn name(&self) -> &Name {
    &self.name
  }

  pub const fn created_at(&self) -> Timestamp {
    self.created_at
  }
}

/// The single editable draft of a resource. `based_on` is the published version it was
/// started from (`None` for a resource that was never published).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft<B> {
  body: B,
  based_on: Option<VersionNumber>,
  updated_at: Timestamp,
}

impl<B> Draft<B> {
  pub const fn new(body: B, based_on: Option<VersionNumber>, updated_at: Timestamp) -> Self {
    Self {
      body,
      based_on,
      updated_at,
    }
  }

  pub const fn body(&self) -> &B {
    &self.body
  }

  pub const fn based_on(&self) -> Option<VersionNumber> {
    self.based_on
  }

  pub const fn updated_at(&self) -> Timestamp {
    self.updated_at
  }

  /// Replaces the body, keeping the version it is based on.
  pub fn edit(&mut self, body: B, now: Timestamp) {
    self.body = body;
    self.updated_at = now;
  }
}

/// An immutable published version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version<B> {
  id: VersionId,
  number: VersionNumber,
  body: B,
  published_at: Timestamp,
}

impl<B> Version<B> {
  pub const fn new(id: VersionId, number: VersionNumber, body: B, published_at: Timestamp) -> Self {
    Self {
      id,
      number,
      body,
      published_at,
    }
  }

  pub const fn id(&self) -> VersionId {
    self.id
  }

  pub const fn number(&self) -> VersionNumber {
    self.number
  }

  pub const fn body(&self) -> &B {
    &self.body
  }

  pub const fn published_at(&self) -> Timestamp {
    self.published_at
  }
}

impl<B: Clone> Version<B> {
  /// Starts a new draft from this version.
  pub fn to_draft(&self, now: Timestamp) -> Draft<B> {
    Draft::new(self.body.clone(), Some(self.number), now)
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublishError {
  /// Another version was published after this draft was started; publishing it would
  /// silently discard that version's changes.
  #[error("draft is based on {based_on:?} but the latest published version is {latest:?}")]
  StaleDraft {
    based_on: Option<VersionNumber>,
    latest: Option<VersionNumber>,
  },
  #[error("the resource has reached the maximum number of versions")]
  VersionLimit,
}

/// Publishes `draft` as the version after `latest`. The caller persists the version and
/// removes the draft atomically.
pub fn publish<B>(draft: Draft<B>, latest: Option<VersionNumber>, now: Timestamp) -> Result<Version<B>, PublishError> {
  if draft.based_on != latest {
    return Err(PublishError::StaleDraft {
      based_on: draft.based_on,
      latest,
    });
  }
  let number = match latest {
    None => VersionNumber::FIRST,
    Some(latest) => latest.next().ok_or(PublishError::VersionLimit)?,
  };
  Ok(Version::new(VersionId::generate(), number, draft.body, now))
}
