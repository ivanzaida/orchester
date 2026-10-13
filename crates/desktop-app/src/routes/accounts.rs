//! Settings › Accounts (`docs/design/agent-accounts.md`, layer `hgOl2`): which account it shows and what its right
//! panel holds, so each state has an address (`/settings?tab=accounts&account=…&panel=used_by`).

use orchester_core::id::AgentAccountId;

use super::{
  RouteError,
  query::{Params, with_query},
};

const ACCOUNT: &str = "account";
const PANEL: &str = "panel";

/// What the right panel of Settings › Accounts holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AccountsPanel {
  /// The account's details, its *Account* tab (`t098v`).
  #[default]
  Account,
  /// Its *Used by* tab (`HzJSR`).
  UsedBy,
  /// *Add account*, step 1: CLI, name and folder (`IOyJl`).
  Add,
  /// Step 2 of *Add account* for the account it shows: sign in with the CLI's own login (`ohqHW`).
  SignIn,
  /// *Remove …?* in place, over its *Used by* tab (`HzJSR`).
  Remove,
  /// The name as a field in the header (`yEJPZ`).
  Rename,
}

impl AccountsPanel {
  const ALL: [Self; 6] = [
    Self::Account,
    Self::UsedBy,
    Self::Add,
    Self::SignIn,
    Self::Remove,
    Self::Rename,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Account => "account",
      Self::UsedBy => "used_by",
      Self::Add => "add",
      Self::SignIn => "sign_in",
      Self::Remove => "remove",
      Self::Rename => "rename",
    }
  }

  fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|panel| panel.name() == name)
  }
}

/// The account Settings › Accounts shows (`None`: the first listed) and its panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AccountsView {
  pub account: Option<AgentAccountId>,
  pub panel: AccountsPanel,
}

impl AccountsView {
  pub const fn of(account: AgentAccountId, panel: AccountsPanel) -> Self {
    Self {
      account: Some(account),
      panel,
    }
  }

  pub(super) fn path(self, settings: String, tab: (&str, &str)) -> String {
    let account = self.account.map(|account| account.to_string());
    let panel = (self.panel != AccountsPanel::Account).then(|| self.panel.name());
    with_query(
      settings,
      &[(tab.0, Some(tab.1)), (ACCOUNT, account.as_deref()), (PANEL, panel)],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let account = params
      .take(ACCOUNT)?
      .map(|id| {
        id.parse().map_err(|_| RouteError::InvalidParam {
          param: ACCOUNT.to_owned(),
          value: id,
        })
      })
      .transpose()?;
    let panel = params.take_or_default(PANEL, AccountsPanel::from_name)?;
    Ok(Self { account, panel })
  }
}
