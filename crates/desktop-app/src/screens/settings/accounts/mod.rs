//! Settings › Accounts (`03 · Accounts` `t8i6m` in the new `orchester.pen`, `docs/design/agent-accounts.md`; its
//! states on `poD7g`): the person's agent CLI accounts on this computer, grouped by CLI (`list`). An account opens its
//! page under the tab (`03b · Account` `xN1Jy`, `details`), and *Add account* its own (`03c · Add account` `XAm8i`,
//! `add`), then the CLI's sign-in (`sign_in`); each has its crumb back to the list, and nothing asks in a dialog.
//! Every account is checked when the tab opens. Adding, renaming, removing and signing in are a person's: while the
//! tab is mounted, the app's MCP server refuses window input (`mcp::input_gate`).

mod add;
mod details;
mod facts;
mod limits;
mod list;
mod remove;
mod sign_in;
mod state;
mod used_by;
mod words;
mod work;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::{account::AgentAccount, id::AgentAccountId, time::Timestamp};
use orchester_workspace::{accounts::AccountUses, background, messages::Message};

use self::work::{ChangeAction, Changed, CheckAction, SignInAction};
use super::frame;
use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  services::Services,
};

#[derive(Clone)]
pub struct Props {
  pub view: AccountsView,
  pub accounts: Arc<Vec<AgentAccount>>,
  /// Counts the changes to the accounts; the screen reads them again on each.
  pub revision: Signal<u64>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.view == other.view && self.accounts == other.accounts
  }
}

impl DevtoolsInspectable for Props {}

/// Accounts by id: those whose check runs now.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Ids(Vec<AgentAccountId>);
impl DevtoolsInspectable for Ids {}

/// The account whose sign-in terminal is open, and when it opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Opened(Option<(AgentAccountId, Timestamp)>);
impl DevtoolsInspectable for Opened {}

/// The account the last *Check now* ran for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PlanChecked(Option<AgentAccountId>);
impl DevtoolsInspectable for PlanChecked {}

/// Where an account's *Check now* stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PlanCheck {
  /// Not run for it, or it gave a reading.
  Idle,
  Running,
  /// It gave no reading, and why.
  Failed(Message),
}

/// What the parts of the tab show.
#[derive(Clone)]
struct Parts {
  pub accounts: Arc<Vec<AgentAccount>>,
  pub view: AccountsView,
  /// The account whose page shows: the one the route names, while it is listed.
  pub selected: Option<AgentAccountId>,
  /// The accounts whose check runs now.
  pub checking: Vec<AgentAccountId>,
  /// The account whose sign-in terminal is open now.
  pub signing_in: Option<AgentAccountId>,
}

impl Parts {
  /// How `account` reads now: *Checking…* or *Signing in…* while Orchester asks its CLI, else its state.
  pub fn shown(&self, account: &AgentAccount) -> words::Shown {
    if self.signing_in == Some(account.id()) {
      words::Shown::SigningIn
    } else if self.checking.contains(&account.id()) {
      words::Shown::Checking
    } else {
      words::Shown::State(account.state())
    }
  }

  pub fn account(&self, id: AgentAccountId) -> Option<&AgentAccount> {
    self.accounts.iter().find(|account| account.id() == id)
  }
}

/// The actions of the tab and the person's choices in its panels.
#[derive(Clone)]
struct Work {
  pub change: ChangeAction,
  pub check: CheckAction,
  pub sign_in: SignInAction,
  pub choose: work::ChooseAction,
  /// *Check now* of a Claude Code account's plan limits, and the account it ran for last.
  plan_check: work::PlanCheckAction,
  plan_checked: Signal<PlanChecked>,
  /// The account whose sign-in terminal opened last, and when.
  opened: Signal<Opened>,
  checking: Signal<Ids>,
  pub draft: add::Draft,
  pub rename: Signal<String>,
  pub removal: remove::Choice,
  /// The account page's *…* menu: whether it is open, and the button it opens at.
  pub more: (Signal<bool>, ElementRef),
}

impl Work {
  /// Checks `accounts` again.
  pub fn check_again(&self, accounts: Vec<AgentAccountId>) {
    self.checking.set(Ids(accounts.clone()));
    self.check.run(accounts);
  }

  /// Runs *Check now* for `account`.
  pub fn check_plan(&self, account: AgentAccountId) {
    self.plan_checked.set(PlanChecked(Some(account)));
    self.plan_check.run(account);
  }

  /// Where `account`'s *Check now* stands.
  pub fn plan_check_of(&self, account: AgentAccountId) -> PlanCheck {
    if self.plan_checked.get().0 != Some(account) {
      return PlanCheck::Idle;
    }
    let state = self.plan_check.state().get();
    match (state.is_pending(), state.error) {
      (true, _) => PlanCheck::Running,
      (false, Some(error)) => PlanCheck::Failed(error),
      (false, None) => PlanCheck::Idle,
    }
  }

  /// When the sign-in terminal of `account` opened, if it is the last one opened.
  pub fn opened_at(&self, account: AgentAccountId) -> Option<Timestamp> {
    self
      .opened
      .get()
      .0
      .filter(|(opened, _)| *opened == account)
      .map(|(_, at)| at)
  }

  /// Whether another account than `account` has a sign-in terminal open: one terminal at a time, so the check that
  /// follows each one's close always reaches the tab.
  pub fn another_signing_in(&self, account: AgentAccountId) -> bool {
    self.sign_in.state().get().is_pending() && self.opened.get().0.is_some_and(|(opened, _)| opened != account)
  }

  /// Opens the sign-in terminal of `account`, unless another account's is open ([`Self::another_signing_in`]), which
  /// its buttons then say.
  pub fn open_sign_in(&self, account: AgentAccountId) {
    if self.another_signing_in(account) {
      return;
    }
    self.opened.set(Opened(Some((account, Timestamp::now()))));
    self.sign_in.run(account);
  }

  /// The error of the last change, while it shows.
  pub fn change_error(&self) -> Option<Message> {
    let state = self.change.state().get();
    let pending = state.is_pending();
    state.error.filter(|_| !pending)
  }
}

pub struct AccountsTab {
  work: Work,
  /// Whether the tab asked every account's CLI once since it opened.
  started: Signal<bool>,
  _protected: Protected,
}

impl Component for AccountsTab {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    let work = Work {
      change: work::change(ctx),
      check: work::check(ctx),
      sign_in: work::sign_in(ctx),
      choose: work::choose_folder(ctx),
      plan_check: work::plan_check(ctx),
      plan_checked: ctx.signal(PlanChecked::default()),
      opened: ctx.signal(Opened::default()),
      checking: ctx.signal(Ids::default()),
      draft: add::Draft::new(ctx),
      rename: ctx.signal(String::new()),
      removal: remove::Choice::new(ctx),
      more: (ctx.signal(false), ElementRef::default()),
    };
    follow(ctx, &work);
    Self {
      work,
      started: ctx.signal(false),
      _protected: gate.protect(Guarded::Accounts),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    if !self.started.get() {
      self.started.set(true);
      let checked = props.accounts.iter().map(AgentAccount::id).collect();
      self.work.check_again(checked);
    }
    // The account whose page shows: the one the route names, while it is listed.
    let selected = props
      .view
      .account
      .filter(|id| props.accounts.iter().any(|account| account.id() == *id));
    let check = self.work.check.state().get();
    let sign_in = self.work.sign_in.state().get();
    let parts = Parts {
      accounts: Arc::clone(&props.accounts),
      view: props.view,
      selected,
      checking: if check.is_pending() {
        self.work.checking.get().0
      } else {
        Vec::new()
      },
      signing_in: sign_in
        .is_pending()
        .then(|| self.work.opened.get().0.map(|(account, _)| account))
        .flatten(),
    };
    let (uses, reading) = uses(ctx, selected, props.revision.get());
    let (scroll, pane) = match (props.view.panel, selected) {
      (AccountsPanel::Add, _) => ("settings-accounts-add-scroll", add::page(ctx, &self.work)),
      (AccountsPanel::SignIn, Some(_)) => (
        "settings-accounts-sign-in-scroll",
        sign_in::page(ctx, &parts, &self.work),
      ),
      (_, Some(_)) => (
        "settings-account-scroll",
        details::page(ctx, &parts, &self.work, (uses.as_ref(), reading)),
      ),
      (_, None) => ("settings-accounts-scroll", list::page(ctx, &parts)),
    };
    frame::column(scroll, pane)
  }
}

/// What uses the shown account, read again after every change, and whether that read is still running.
fn uses(ctx: &mut Ctx, account: Option<AgentAccountId>, revision: u64) -> (Option<AccountUses>, bool) {
  let workspace = Services::of(ctx).workspace;
  let state = ctx
    .future((account, revision), move |(account, _)| {
      let workspace = workspace.clone();
      async move {
        let Some(account) = account else {
          return Ok(None);
        };
        background::run(move || workspace.agent_account_uses(account))
          .await?
          .map(Some)
          .map_err(|error| error.message())
      }
    })
    .state()
    .get();
  let reading = state.is_pending();
  (state.data.flatten(), reading)
}

/// After a change the accounts are read again and the tab shows what it changed; a new account in a folder
/// Orchester created goes on to its sign-in.
fn follow(ctx: &mut Ctx, work: &Work) {
  let revision = ctx.props::<Props>().revision.clone();
  let navigation = Navigation::of(ctx);
  let sign_in = work.clone();
  let bump = revision.clone();
  ctx.watch(&work.change.state(), move |state| {
    let Some(changed) = state.data.filter(|_| state.is_fulfilled()) else {
      return;
    };
    bump.update(|revision| *revision += 1);
    let view = match changed {
      Changed::Added(account, true) => {
        sign_in.open_sign_in(account);
        AccountsView::of(account, AccountsPanel::SignIn)
      }
      Changed::Added(account, false) | Changed::Renamed(account) | Changed::FolderChosen(account) => {
        AccountsView::of(account, AccountsPanel::Account)
      }
      Changed::Removed => AccountsView::default(),
    };
    navigation.replace(Route::Accounts(view));
  });
  let bump = revision.clone();
  ctx.watch(&work.check.state(), move |state| {
    if !state.is_pending() {
      bump.update(|revision| *revision += 1);
    }
  });
  let change = work.change.clone();
  ctx.watch(&work.choose.state(), move |state| {
    if let Some(work::ChosenFolder(Some((account, folder)))) = state.data.clone().filter(|_| state.is_fulfilled()) {
      change.run(work::Change::ChooseFolder(account, folder));
    }
  });
  ctx.watch(&work.sign_in.state(), move |state| {
    if !state.is_pending() {
      revision.update(|revision| *revision += 1);
    }
  });
}
