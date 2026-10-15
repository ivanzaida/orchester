//! A server project's address, its identity since API 3 (`docs/architecture/server.md`, rows 49–50, "Servers and
//! project addresses"): the team server's address and the project's slug, `https://orchester.example.com/kontur`.
//! The repository a project was set up for is only a hint: linking a checkout whose `origin` is another repository is
//! allowed, with a warning ([`remote_match`]).

use orchester_api::client::{AddressError, ServerAddress};
use orchester_core::{repository::RepositoryIdentity, resource::Slug};

/// Why a typed project address is not one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectAddressError {
  #[error(transparent)]
  Address(#[from] AddressError),
  /// The address names no project after the server's: its last segment is not a slug.
  #[error("the address does not end in a project's slug")]
  NoSlug,
}

/// A project's address: the server it is on and its slug there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectAddress {
  server: ServerAddress,
  slug: Slug,
}

impl ProjectAddress {
  /// Parses what the person typed or `orchester.toml` names: a server address with the slug as its last segment.
  pub fn parse(text: &str) -> Result<Self, ProjectAddressError> {
    let whole = ServerAddress::parse(text)?;
    let (server, slug) = whole
      .as_str()
      .rsplit_once('/')
      .filter(|(server, _)| !server.ends_with('/'))
      .ok_or(ProjectAddressError::NoSlug)?;
    let slug = Slug::new(slug).map_err(|_| ProjectAddressError::NoSlug)?;
    Ok(Self {
      server: ServerAddress::parse(server)?,
      slug,
    })
  }

  pub fn server(&self) -> &ServerAddress {
    &self.server
  }

  pub fn slug(&self) -> &str {
    self.slug.as_str()
  }

  /// The address as people share it, `https://orchester.example.com/kontur`.
  pub fn to_text(&self) -> String {
    format!("{}/{}", self.server.as_str(), self.slug.as_str())
  }
}

/// How a checkout's `origin` compares with the repository a server project was set up for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteMatch {
  /// The same repository.
  Same,
  /// Another repository: linking warns and the person may go on.
  Different {
    checkout: RepositoryIdentity,
    hint: RepositoryIdentity,
  },
  /// The project names no repository; nothing to compare.
  NoHint,
  /// The checkout has no network `origin`; it can still be linked.
  NoOrigin,
}

impl RemoteMatch {
  /// Whether linking shows the mismatch warning.
  pub const fn warns(&self) -> bool {
    matches!(self, Self::Different { .. })
  }
}

/// The rule of row 49: a checkout's `origin` (`None` without a network one) against the project's remote hint.
pub fn remote_match(checkout: Option<&RepositoryIdentity>, hint: Option<&RepositoryIdentity>) -> RemoteMatch {
  match (checkout, hint) {
    (_, None) => RemoteMatch::NoHint,
    (None, Some(_)) => RemoteMatch::NoOrigin,
    (Some(checkout), Some(hint)) if checkout == hint => RemoteMatch::Same,
    (Some(checkout), Some(hint)) => RemoteMatch::Different {
      checkout: checkout.clone(),
      hint: hint.clone(),
    },
  }
}
