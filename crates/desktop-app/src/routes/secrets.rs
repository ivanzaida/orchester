//! What Project settings › Environment shows (`06 Secrets` `bVh1e`), so each filter, secret and question has an address
//! (`/project/<id>/secrets?environment=staging&secret=…&panel=replace`).

use orchester_core::{id::SecretId, secret::Environment};

use super::{
  RouteError,
  query::{Params, with_query},
};

const ENVIRONMENT: &str = "environment";
const SECRET: &str = "secret";
const PANEL: &str = "panel";

/// What the tab shows besides its list or a secret's page: the list or the page itself, the one it opens on, or a form
/// or a question.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SecretsPanel {
  #[default]
  Inspect,
  /// *New secret*, a page of its own (`YGy5i`).
  Add,
  /// *Replace value* of the secret shown, in place on its page (`yTCmk`).
  Replace,
  /// *Archive …?* in place on the secret's page (`fw0eT`).
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

/// The environment the list is filtered to (`None`: every one), the secret whose page shows (`None`: the list) and
/// what shows besides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SecretsView {
  pub environment: Option<Environment>,
  pub secret: Option<SecretId>,
  pub panel: SecretsPanel,
}

impl SecretsView {
  pub(super) fn path(self, secrets: String) -> String {
    let secret = self.secret.map(|id| id.to_string());
    let panel = (self.panel != SecretsPanel::default()).then(|| self.panel.name());
    with_query(
      secrets,
      &[
        (ENVIRONMENT, self.environment.map(Environment::as_str)),
        (SECRET, secret.as_deref()),
        (PANEL, panel),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    Ok(Self {
      environment: params.take_parsed(ENVIRONMENT)?,
      secret: params.take_parsed(SECRET)?,
      panel: params.take_or_default(PANEL, SecretsPanel::from_name)?,
    })
  }
}
