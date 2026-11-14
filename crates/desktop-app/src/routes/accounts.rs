//! Settings › Accounts (`docs/design/agent-accounts.md`; `03 · Accounts` `t8i6m` in the new `orchester.pen`): the
//! list, or the account page it opens and what that page asks, so each state has an address
//! (`/settings?tab=accounts&account=…&panel=remove`). An API provider's page is addressed by the provider
//! (`&provider=…`; `Agents on accounts` B1 `gZTuX`).

use orchester_core::id::{AgentAccountId, ApiAccountId};

use super::{
  RouteError,
  query::{Params, with_query},
};

const ACCOUNT: &str = "account";
const PROVIDER: &str = "provider";
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
  /// *Add an API provider* (`Agents on accounts` B2 `OfUhN`): an agent CLI on a model API, with a key the app keeps.
  AddApi,
  /// *Remove …?* in place on the account's page (`poD7g`).
  Remove,
  /// The name as a field in the page's head (`poD7g`).
  Rename,
  /// An API provider's base URL and models as fields on its page.
  Edit,
}

impl AccountsPanel {
  pub const ALL: [Self; 8] = [
    Self::Account,
    Self::UsedBy,
    Self::Add,
    Self::SignIn,
    Self::AddApi,
    Self::Remove,
    Self::Rename,
    Self::Edit,
  ];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Account => "account",
      Self::UsedBy => "used_by",
      Self::Add => "add",
      Self::SignIn => "sign_in",
      Self::AddApi => "add_api",
      Self::Remove => "remove",
      Self::Rename => "rename",
      Self::Edit => "edit",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|panel| panel.name() == name)
  }
}

/// The account Settings › Accounts shows (`None`: the first listed), or the API provider, and its panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AccountsView {
  pub account: Option<AgentAccountId>,
  /// The API provider whose page shows; a sign-in's page shows when both are named.
  pub provider: Option<ApiAccountId>,
  pub panel: AccountsPanel,
}

impl AccountsView {
  pub const fn of(account: AgentAccountId, panel: AccountsPanel) -> Self {
    Self {
      account: Some(account),
      provider: None,
      panel,
    }
  }

  /// The page of the API provider `provider` on `panel`.
  pub const fn of_provider(provider: ApiAccountId, panel: AccountsPanel) -> Self {
    Self {
      account: None,
      provider: Some(provider),
      panel,
    }
  }

  /// The list with `panel` over it: *Add account* or *Add an API provider*.
  pub const fn panel(panel: AccountsPanel) -> Self {
    Self {
      account: None,
      provider: None,
      panel,
    }
  }

  pub(super) fn path(self, settings: String, tab: (&str, &str)) -> String {
    let account = self.account.map(|account| account.to_string());
    let provider = self.provider.map(|provider| provider.to_string());
    let panel = (self.panel != AccountsPanel::Account).then(|| self.panel.name());
    with_query(
      settings,
      &[
        (tab.0, Some(tab.1)),
        (ACCOUNT, account.as_deref()),
        (PROVIDER, provider.as_deref()),
        (PANEL, panel),
      ],
    )
  }

  pub(super) fn take(params: &mut Params) -> Result<Self, RouteError> {
    let account = id_of(params, ACCOUNT)?;
    let provider = id_of(params, PROVIDER)?;
    let panel = params.take_or_default(PANEL, AccountsPanel::from_name)?;
    Ok(Self {
      account,
      provider,
      panel,
    })
  }
}

/// The id the query names under `param`, when it names one.
fn id_of<T: std::str::FromStr>(params: &mut Params, param: &str) -> Result<Option<T>, RouteError> {
  params
    .take(param)?
    .map(|id| {
      id.parse().map_err(|_| RouteError::InvalidParam {
        param: param.to_owned(),
        value: id,
      })
    })
    .transpose()
}
