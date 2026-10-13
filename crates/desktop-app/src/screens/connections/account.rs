//! A connection's *Account* (`docs/design/agent-accounts.md`, `WaKJN` and `M42rM`): whose sign-in the connection's CLI
//! runs on, chosen among the person's accounts of that CLI, each with who is signed in, the plan and its weekly use,
//! or its state; *Add or manage accounts…* opens Settings › Accounts. Accounts are app-wide: a project only picks one.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  account::{AccountState, AgentAccount},
  connection::{
    AgentProvider, ConnectionAccount,
    invocation::Platform,
    plan_limits::{LimitReading, PlanReadings},
  },
  time::Timestamp,
};

use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{AccountsView, Route},
  services::accounts::Accounts,
  ui::{
    choice_field::{Choice, ChoiceField},
    icon::Icon,
  },
};

/// The choice that opens Settings › Accounts instead of choosing an account.
pub(super) const MANAGE: &str = "manage";
/// A week, in minutes: the window a choice's use is read from.
const WEEK: u64 = 10_080;

/// The account's name as a connection shows it: the one given, or *Default*.
pub(super) fn name(ctx: &Ctx, account: &AgentAccount) -> String {
  account.name().map_or_else(
    || ctx.t("connections.account.default").to_string(),
    |name| name.as_str().to_owned(),
  )
}

/// The name of the account `account` names for a connection of `provider`: *No account* without one.
pub(super) fn named(
  ctx: &Ctx,
  accounts: &Accounts,
  account: &ConnectionAccount,
  provider: AgentProvider,
) -> Option<String> {
  if provider.is_native() {
    return None;
  }
  match accounts.of_connection(account, provider) {
    Some(account) => Some(name(ctx, account)),
    None if *account == ConnectionAccount::Unassigned => Some(ctx.t("connections.account.none").to_string()),
    None => None,
  }
}

/// What an account choice says under its name: who, the plan and the week's use, or what is wrong.
fn detail(ctx: &Ctx, account: &AgentAccount, reading: Option<&LimitReading>) -> String {
  let state = account.state(Platform::current());
  let state_key = match state {
    AccountState::SignedIn | AccountState::Unknown | AccountState::NotChecked => None,
    AccountState::NotSignedIn => Some("connections.account.state.notSignedIn"),
    AccountState::Expired => Some("connections.account.state.expired"),
    AccountState::CliMissing => Some("connections.account.state.cliMissing"),
    AccountState::FolderMissing => Some("connections.account.state.folderMissing"),
    AccountState::Unsupported => Some("connections.account.state.unsupported"),
  };
  let identity = account.last_check().and_then(|check| check.known_identity());
  let mut parts: Vec<String> = identity
    .and_then(|identity| identity.email.clone())
    .into_iter()
    .collect();
  if let Some(key) = state_key {
    parts.push(ctx.t(key).to_string());
    return parts.join(" · ");
  }
  if let Some(plan) = identity.and_then(|identity| identity.plan.as_deref()) {
    parts.push(ctx.t_args("connections.account.plan", [("plan", plan)]).to_string());
  }
  let now = Timestamp::now();
  let weekly = reading.and_then(|reading| {
    reading
      .limits
      .windows
      .iter()
      .find(|window| window.minutes() == WEEK && !window.expired(now))
  });
  if let Some(used) = weekly.and_then(orchester_core::connection::plan_limits::LimitWindow::percent) {
    // A share of at most a few hundred percent: whole ones are what the menu shows.
    let used = used.round() as u64;
    parts.push(
      ctx
        .t_args("connections.account.weekly", [("used", used.to_string())])
        .to_string(),
    );
  }
  parts.join(" · ")
}

/// The choices of the accounts of `provider`'s CLI, then *Add or manage accounts…*.
pub(super) fn choices(ctx: &Ctx, accounts: &Accounts, readings: &PlanReadings, provider: AgentProvider) -> Vec<Choice> {
  let mut choices: Vec<Choice> = accounts
    .of_provider(provider)
    .map(|account| Choice {
      value: account.id().to_string(),
      label: name(ctx, account).into(),
      icon: Icon::User,
      detail: Some(detail(ctx, account, readings.of_account(account.id())).into())
        .filter(|detail: &Arc<str>| !detail.is_empty()),
      separator: false,
    })
    .collect();
  choices.push(Choice {
    value: MANAGE.to_owned(),
    label: ctx.t("connections.account.manage"),
    icon: Icon::Settings,
    detail: Some(ctx.t("connections.account.manageDetail")),
    separator: true,
  });
  choices
}

/// An account picker's props: its choices, the account chosen now (by id), and what choosing another does.
#[derive(Clone)]
pub(super) struct PickerProps {
  pub id: Arc<str>,
  pub name: Arc<str>,
  pub choices: Vec<Choice>,
  pub current: String,
  pub width: f32,
  pub on_choose: Arc<dyn Fn(String) + Send + Sync>,
}

impl PartialEq for PickerProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.id, &self.name, &self.choices, &self.current, self.width)
      == (&other.id, &other.name, &other.choices, &other.current, other.width)
  }
}

impl DevtoolsInspectable for PickerProps {}

/// The *Account* field: a menu of the accounts; choosing *Add or manage accounts…* opens Settings › Accounts.
/// Which account a connection's agents run on is a person's decision: lurq's synthetic input (an MCP client driving
/// the window) chooses one only under the stored consent to let automation change agent permissions, as
/// `set_connection_account` does; otherwise the choice is not taken and the field shows the account again.
pub(super) struct AccountPicker {
  value: Signal<String>,
  open: Signal<bool>,
  /// The account the props say is chosen, which the menu shows without choosing it again.
  current: Signal<String>,
}

impl Component for AccountPicker {
  type Props = PickerProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<PickerProps>().clone();
    let picker = Self {
      value: ctx.signal(props.current.clone()),
      open: ctx.signal(false),
      current: ctx.signal(props.current.clone()),
    };
    let navigation = Navigation::of(ctx);
    let gate = ctx.use_context::<InputGate>();
    let (value, current, on_choose) = (picker.value.clone(), picker.current.clone(), props.on_choose);
    ctx.watch(&picker.value, move |chosen| {
      let shown = current.get_untracked();
      if chosen == MANAGE {
        value.set(shown);
        navigation.go(Route::Accounts(AccountsView::default()));
      } else if *chosen != shown {
        if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
          on_choose(chosen.clone());
        } else {
          tracing::warn!("synthetic input chose a connection's account; not taken");
          value.set(shown);
        }
      }
    });
    picker
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<PickerProps>().clone();
    if self.current.get_untracked() != props.current {
      self.current.set(props.current.clone());
      self.value.set(props.current.clone());
    }
    ChoiceField {
      id: props.id.clone(),
      name: props.name.clone(),
      value: self.value.clone(),
      open: self.open.clone(),
      choices: props.choices.clone(),
      width: props.width,
    }
    .render(ctx)
  }
}

/// The *Account* row of Add connection, after *Program*: the accounts of `provider`'s CLI, `chosen` by id (empty:
/// Default).
pub(super) fn add_row(ctx: &mut Ctx, accounts: &Accounts, provider: AgentProvider, chosen: &Signal<String>) -> Element {
  let readings = crate::services::plan_limits::readings(ctx);
  let current =
    chosen_account(accounts, provider, &chosen.get()).map_or_else(String::new, |account| account.id().to_string());
  let setter = chosen.clone();
  let picker = ctx.mount_keyed::<AccountPicker>(
    "connection-account-picker",
    PickerProps {
      id: "connection-account".into(),
      name: ctx.t("connections.account.label"),
      choices: choices(ctx, accounts, &readings, provider),
      current,
      width: super::page::FORM_WIDTH,
      on_choose: Arc::new(move |chosen| setter.set(chosen)),
    },
  );
  crate::ui::form_section::stacked_row(
    crate::ui::form_section::Label::new("connection-account", ctx.t("connections.account.label"))
      .description(ctx.t("connections.account.description")),
    picker,
    crate::ui::form_section::Place::Within,
  )
}

/// The account of `provider`'s CLI with the id `chosen`, else its Default.
pub(super) fn chosen_account<'a>(
  accounts: &'a Accounts,
  provider: AgentProvider,
  chosen: &str,
) -> Option<&'a AgentAccount> {
  accounts
    .of_provider(provider)
    .find(|account| account.id().to_string() == chosen)
    .or_else(|| accounts.of_provider(provider).find(|account| account.is_default()))
}

/// The *Account* row of a CLI connection's inspector (`M42rM`): the account it runs on, which the person can change;
/// another account is saved and the connection checked there.
pub(super) fn inspector_row(
  ctx: &mut Ctx,
  accounts: &Accounts,
  connection: &orchester_core::connection::AgentConnection,
  choose: Arc<dyn Fn(orchester_core::id::AgentAccountId) + Send + Sync>,
) -> Element {
  let provider = connection.provider();
  let readings = crate::services::plan_limits::readings(ctx);
  let current = accounts
    .of_connection(connection.account(), provider)
    .map_or_else(String::new, |account| account.id().to_string());
  let picker = ctx.mount_keyed::<AccountPicker>(
    &format!("connection-account-picker-{}", connection.id()),
    PickerProps {
      id: "connection-account".into(),
      name: ctx.t("connections.account.label"),
      choices: choices(ctx, accounts, &readings, provider),
      current,
      width: INSPECTOR_PICKER_WIDTH,
      on_choose: Arc::new(move |chosen| {
        if let Ok(account) = chosen.parse() {
          choose(account);
        }
      }),
    },
  );
  lurq::components::Column::new()
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .child(crate::ui::inspector::control(
      "connection-account-row",
      &ctx.t("connections.account.label"),
      picker,
    ))
    .child(crate::ui::inspector::note(
      "connection-account-note",
      Icon::Info,
      &ctx.t("connections.account.changeNote"),
    ))
    .into()
}

/// The picker's width in an inspector (`Account` row of `M42rM`).
const INSPECTOR_PICKER_WIDTH: f32 = 220.0;
