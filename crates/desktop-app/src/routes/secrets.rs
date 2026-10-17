//! What Secrets shows (`ccz8n`), so each tab, filter, secret and panel has an address
//! (`/project/<id>/secrets?environment=staging&secret=…&panel=replace`, `?tab=usage`).

use orchester_core::{id::SecretId, secret::Environment};

use super::{
  RouteError, TAB,
  query::{Params, with_query},
};

const ENVIRONMENT: &str = "environment";
const SECRET: &str = "secret";
const PANEL: &str = "panel";

/// Secrets' tabs (`List` and `Usage`); the list is the one it opens on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SecretsTab {
  #[default]
  List,
  Usage,
}

impl SecretsTab {
  const ALL: [Self; 2] = [Self::List, Self::Usage];

  pub const fn name(self) -> &'static str {
    match self {
      Self::List => "list",
      Self::Usage => "usage",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}

/// What the list's right panel holds: the secret's inspector, the one it opens on, or a form over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SecretsPanel {
  #[default]
  Inspect,
  /// *New secret* (`YGy5i`).
  Add,
  /// *Replace value* of the secret shown (`yTCmk`).
  Replace,
  /// *Archive …?* in place, over the secret's inspector (`K2epbr`).
  Archive,
}

impl SecretsPanel {
  const ALL: [Self; 4] = [Self::Inspect, Self::Add, Self::Replace, Self::Archive];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Inspect => "inspect",
      Self::Add => "add",
      Self::Replace => "replace",
      Self::Archive => "archive",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|panel| panel.name() == name)
  }
}

/// Secrets' tab, the environment its list is filtered to (`None`: every one), the secret chosen (`None`: the
/// first listed) and the right panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SecretsView {
  pub tab: SecretsTab,
  pub environment: Option<Environment>,
  pub secret: Option<SecretId>,
  pub panel: SecretsPanel,
}

impl SecretsView {
  pub(super) fn path(self, secrets: String) -> String {
    let tab = (self.tab != SecretsTab::default()).then(|| self.tab.name());
    let secret = self.secret.map(|id| id.to_string());
    let panel = (self.panel != SecretsPanel::default()).then(|| self.panel.name());
    with_query(
      secrets,
      &[
        (TAB, tab),
        (ENVIRONMENT, self.environment.map(Environment::as_str)),
        (SECRET, secret.as_deref()),
        (PANEL, panel),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    Ok(Self {
      tab: params.take_or_default(TAB, SecretsTab::from_name)?,
      environment: params.take_parsed(ENVIRONMENT)?,
      secret: params.take_parsed(SECRET)?,
      panel: params.take_or_default(PANEL, SecretsPanel::from_name)?,
    })
  }
}
