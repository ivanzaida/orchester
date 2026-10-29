//! The agent profile's *Browser* permission (`docs/design/browser.md`, "Permissions and policy"): off by default; on,
//! the agent gets Orchester's browser tools ([`crate::orchester_tools::browser`]) while it also has network access, and
//! opens and acts on the allowed sites and the local development hosts without asking. Turning it on or adding a site
//! widens what agents may do ([`super::widening::Widening`]), which over MCP only a person does.

mod site;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::site::{AllowedSite, AllowedSiteError, SiteHost};

/// Whether the profile's agents get the browser.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserAccess {
  #[default]
  Off,
  On,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AllowedSitesError {
  #[error("`{0}` is listed more than once")]
  Duplicate(AllowedSite),
  #[error("at most {max} sites can be listed", max = AllowedSites::MAX_ENTRIES)]
  TooMany,
}

/// The sites an agent may open and act on without asking, in the order they were added.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "Vec<AllowedSite>", into = "Vec<AllowedSite>")]
pub struct AllowedSites(Vec<AllowedSite>);

impl AllowedSites {
  pub const MAX_ENTRIES: usize = 50;

  pub const fn none() -> Self {
    Self(Vec::new())
  }

  pub fn new(entries: Vec<AllowedSite>) -> Result<Self, AllowedSitesError> {
    if entries.len() > Self::MAX_ENTRIES {
      return Err(AllowedSitesError::TooMany);
    }
    for (index, entry) in entries.iter().enumerate() {
      if entries[..index].contains(entry) {
        return Err(AllowedSitesError::Duplicate(entry.clone()));
      }
    }
    Ok(Self(entries))
  }

  pub fn entries(&self) -> &[AllowedSite] {
    &self.0
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Whether a listed site is `host`'s.
  pub fn matches(&self, host: &SiteHost) -> bool {
    self.0.iter().any(|site| site.matches(host))
  }

  /// Whether a listed site covers `site` ([`AllowedSite::covers`]).
  pub fn covers(&self, site: &AllowedSite) -> bool {
    self.0.iter().any(|entry| entry.covers(site))
  }
}

impl TryFrom<Vec<AllowedSite>> for AllowedSites {
  type Error = AllowedSitesError;

  fn try_from(entries: Vec<AllowedSite>) -> Result<Self, AllowedSitesError> {
    Self::new(entries)
  }
}

impl From<AllowedSites> for Vec<AllowedSite> {
  fn from(sites: AllowedSites) -> Self {
    sites.0
  }
}

/// The *Browser* permission: on or off, and the allowed sites, kept while it is off so that turning it on again
/// restores them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BrowserPermission {
  access: BrowserAccess,
  #[serde(default, skip_serializing_if = "AllowedSites::is_empty")]
  sites: AllowedSites,
}

impl BrowserPermission {
  pub const fn new(access: BrowserAccess, sites: AllowedSites) -> Self {
    Self { access, sites }
  }

  /// What profiles saved before the permission existed get: off, no site.
  pub const fn off() -> Self {
    Self::new(BrowserAccess::Off, AllowedSites::none())
  }

  pub const fn access(&self) -> BrowserAccess {
    self.access
  }

  pub const fn sites(&self) -> &AllowedSites {
    &self.sites
  }

  /// Off with no site, as profiles saved before the permission existed: left out of the stored form.
  pub fn is_unset(&self) -> bool {
    *self == Self::off()
  }

  /// Whether the agent opens and acts on a page of `host` without asking: a local development host or a listed site.
  /// Whether the browser is offered at all is the step's question ([`crate::orchester_tools::browser::offered`]).
  pub fn allows_site(&self, host: &SiteHost) -> bool {
    host.is_local() || self.sites.matches(host)
  }
}
