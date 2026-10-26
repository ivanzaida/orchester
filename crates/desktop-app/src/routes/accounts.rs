//! Settings › Accounts (`docs/design/agent-accounts.md`; `03 · Accounts` `t8i6m` in the new `orchester.pen`): the
//! list, or the account page it opens and what that page asks, so each state has an address
//! (`/settings?tab=accounts&account=…&panel=remove`).

use orchester_core::id::AgentAccountId;

use super::{
  RouteError,
  query::{Params, with_query},
};

const ACCOUNT: &str = "account";
const PANEL: &str = "panel";

/// What Settings › Accounts shows over the list, or for the account it opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AccountsPanel {
  /// The account's page with its crumb back to the list (`03b · Account` `xN1Jy`); without an account, the list.
  #[default]
  Account,
  /// The account's page, as [`Self::Account`]: its *Used by* group is on the page now. Kept so older links open.
  UsedBy,
  /// *Add account*, step 1: CLI, name and folder (`03c · Add account` `XAm8i`).
  Add,
  /// Step 2 of *Add account* for the account it shows: sign in with the CLI's own login.
  SignIn,
  /// *Remove …?* in place on the account's page (`poD7g`).
  Remove,
  /// The name as a field in the page's head (`poD7g`).
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
