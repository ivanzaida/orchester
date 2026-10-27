//! Settings › Orchester Cloud (`vPV73`, `C6qnY`, `MrVql` in the new `orchester.pen`): its column, one team server's
//! page or *Connect a server*, at the step it opens on, and the question it asks in place, so each state has an
//! address (`/settings?tab=orchester&server=…&panel=connect&step=compare`, `…&ask=sign-out`,
//! `…&ask=decline&invite=…`). Opening a question is going somewhere; answering it stays a person's.

use orchester_core::id::{InviteId, ServerId};

use super::{
  RouteError,
  query::{Params, with_query},
};

const SERVER: &str = "server";
const PANEL: &str = "panel";
const STEP: &str = "step";
const ASK: &str = "ask";
const INVITE: &str = "invite";
const SIGN_OUT: &str = "sign-out";
const DECLINE: &str = "decline";

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

/// The step *Connect a server* opens on, at the address of the server the route names (`MrVql`, `IBnMu`): past
/// *Address*, the page asks what answers there as *Continue* does. A step only a person's answer reaches stays
/// before it: *Join* waits behind the check code of a server with its own certificate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ConnectStep {
  #[default]
  Address,
  /// *Compare the check code* (`05b2`).
  Compare,
  /// *Join*, with *Your username there* beside it (`05b`).
  Join,
}

impl ConnectStep {
  pub const ALL: [Self; 3] = [Self::Address, Self::Compare, Self::Join];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Address => "address",
      Self::Compare => "compare",
      Self::Join => "join",
    }
  }

  /// The step `name` names; *Your username there* is a part of *Join*.
  pub fn from_name(name: &str) -> Option<Self> {
    match name {
      "username" => Some(Self::Join),
      name => Self::ALL.into_iter().find(|step| step.name() == name),
    }
  }
}

/// A question Settings › Orchester Cloud opens with, in place (`ZUuMT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OrchesterAsk {
  /// *Sign out of Orchester Cloud?* under the account card, or *Sign out of …?* on a team server's page.
  SignOut,
  /// *Decline the invite to …?* in place of the invite's actions.
  Decline(InviteId),
}

/// The team server Settings › Orchester shows (`None`: the cloud account), the panel, the step *Connect a server*
/// opens on and the question asked in place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OrchesterView {
  pub server: Option<ServerId>,
  pub panel: OrchesterPanel,
  pub step: ConnectStep,
  pub ask: Option<OrchesterAsk>,
}

impl OrchesterView {
  /// The entry of `server`.
  pub const fn server(server: ServerId) -> Self {
    Self {
      server: Some(server),
      panel: OrchesterPanel::Account,
      step: ConnectStep::Address,
      ask: None,
    }
  }

  /// *Connect a server*, empty.
  pub const fn connect() -> Self {
    Self {
      server: None,
      panel: OrchesterPanel::Connect,
      step: ConnectStep::Address,
      ask: None,
    }
  }

  /// The same place, asking nothing.
  pub const fn unasked(self) -> Self {
    Self { ask: None, ..self }
  }

  pub(super) fn path(self, settings: String, tab: (&str, &str)) -> String {
    let server = self.server.map(|server| server.to_string());
    let panel = (self.panel != OrchesterPanel::Account).then(|| self.panel.name());
    let step = (self.step != ConnectStep::Address).then(|| self.step.name());
    let (ask, invite) = match self.ask {
      None => (None, None),
      Some(OrchesterAsk::SignOut) => (Some(SIGN_OUT), None),
      Some(OrchesterAsk::Decline(invite)) => (Some(DECLINE), Some(invite.to_string())),
    };
    with_query(
      settings,
      &[
        (tab.0, Some(tab.1)),
        (SERVER, server.as_deref()),
        (PANEL, panel),
        (STEP, step),
        (ASK, ask),
        (INVITE, invite.as_deref()),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let server = params.take_parsed(SERVER)?;
    let panel = params.take_or_default(PANEL, OrchesterPanel::from_name)?;
    let step = params.take_or_default(STEP, ConnectStep::from_name)?;
    let invite: Option<InviteId> = params.take_parsed(INVITE)?;
    let ask = match (params.take(ASK)?.as_deref(), invite) {
      (None, None) => None,
      (Some(SIGN_OUT), None) => Some(OrchesterAsk::SignOut),
      (Some(DECLINE), Some(invite)) => Some(OrchesterAsk::Decline(invite)),
      (Some(other), _) => {
        return Err(RouteError::InvalidParam {
          param: ASK.to_owned(),
          value: other.to_owned(),
        });
      }
      (None, Some(_)) => {
        return Err(RouteError::UnexpectedParam {
          param: INVITE.to_owned(),
        });
      }
    };
    Ok(Self {
      server,
      panel,
      step,
      ask,
    })
  }
}
