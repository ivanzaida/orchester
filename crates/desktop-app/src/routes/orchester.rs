//! Settings › Orchester (`Cu20b`, `SFaAp`, `zxe2P` in the *Settings* layer `uyO5v`): what the right panel shows, the
//! Orchester Cloud account or one team server, and *Connect a server*, so each state has an address
//! (`/settings?tab=orchester&server=…&panel=connect`).

use orchester_core::id::ServerId;

use super::{
  RouteError,
  query::{Params, with_query},
};

const SERVER: &str = "server";
const PANEL: &str = "panel";

/// What Settings › Orchester shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OrchesterPanel {
  /// The cloud account's *Account* tab, or the team server `server` names.
  #[default]
  Account,
  /// The cloud account's *Invites* tab.
  Invites,
  /// *Connect a server* (`zxe2P`), at the address of `server` when it names one (joining it again with a code).
  Connect,
}

impl OrchesterPanel {
  const ALL: [Self; 3] = [Self::Account, Self::Invites, Self::Connect];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Account => "account",
      Self::Invites => "invites",
      Self::Connect => "connect",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|panel| panel.name() == name)
  }
}

/// The team server Settings › Orchester shows (`None`: the cloud account) and the panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OrchesterView {
  pub server: Option<ServerId>,
  pub panel: OrchesterPanel,
}

impl OrchesterView {
  /// The entry of `server`.
  pub const fn server(server: ServerId) -> Self {
    Self {
      server: Some(server),
      panel: OrchesterPanel::Account,
    }
  }

  /// *Connect a server*, empty.
  pub const fn connect() -> Self {
    Self {
      server: None,
      panel: OrchesterPanel::Connect,
    }
  }

  pub(super) fn path(self, settings: String, tab: (&str, &str)) -> String {
    let server = self.server.map(|server| server.to_string());
    let panel = (self.panel != OrchesterPanel::Account).then(|| self.panel.name());
    with_query(
      settings,
      &[(tab.0, Some(tab.1)), (SERVER, server.as_deref()), (PANEL, panel)],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let server = params
      .take(SERVER)?
      .map(|id| {
        id.parse().map_err(|_| RouteError::InvalidParam {
          param: SERVER.to_owned(),
          value: id,
        })
      })
      .transpose()?;
    let panel = params.take_or_default(PANEL, OrchesterPanel::from_name)?;
    Ok(Self { server, panel })
  }
}
