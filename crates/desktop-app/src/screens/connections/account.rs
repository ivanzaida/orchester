//! A connection's *Account* (`docs/design/agent-accounts.md`, `WaKJN` and `M42rM`): whose sign-in the connection's CLI
//! runs on, chosen among the person's accounts of that CLI, each with who is signed in, the plan and its weekly use,
//! or its state; *Add or manage accounts…* opens Settings › Accounts. Accounts are app-wide: a project only picks one.

use std::{path::Path, sync::Arc};

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
};

use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{AccountsView, Route},
  services::accounts::{Accounts, DetailPlace, identity_detail, short_state},
  ui::{
    choice_field::{Choice, ChoiceField},
    icon::Icon,
  },
};

/// The choice that opens Settings › Accounts instead of choosing an account.
pub(super) const MANAGE: &str = "manage";
/// How much of a folder's path a choice's problem shows, its start cut (`WaKJN`).
const PROBLEM_PATH_CHARS: usize = 44;

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

/// What is wrong with `account` for a connection, if anything: the words a menu choice and the inspector use.
fn state_words(ctx: &Ctx, account: &AgentAccount) -> Option<String> {
  let key = match account.state(Platform::current()) {
    AccountState::SignedIn | AccountState::Unknown | AccountState::NotChecked => return None,
    AccountState::NotSignedIn => "connections.account.state.notSignedIn",
    AccountState::Expired => "connections.account.state.expired",
    AccountState::CliMissing => "connections.account.state.cliMissing",
    AccountState::FolderMissing => "connections.account.state.folderMissing",
    AccountState::Unsupported => "connections.account.state.unsupported",
  };
  Some(ctx.t(key).to_string())
}

/// What an account's choice says under its name (`WaKJN`), and whether it says what is wrong: who is signed in,
/// the plan and the week's use; the account it shares a sign-in with; or its folder, what is wrong and where to fix
/// it, in `warning`. An account no check has read yet says so.
fn detail(ctx: &Ctx, account: &AgentAccount, accounts: &Accounts, reading: Option<&LimitReading>) -> (String, bool) {
  if let Some(problem) = state_words(ctx, account) {
    let folder = account
      .folder()
      .path()
      .map(Path::to_path_buf)
      .or_else(|| orchester_workspace::accounts::own_folder(account.cli()))
      .map(|folder| orchester_workspace::display::shorten_path(&folder, PROBLEM_PATH_CHARS))
      .unwrap_or_default();
    let text = ctx.t_args(
      "connections.account.problem",
      [("folder", folder), ("problem", problem)],
    );
    return (text.to_string(), true);
  }
  (
    identity_detail(ctx, (account, accounts), reading, DetailPlace::Line),
    false,
  )
}

/// The choices of the accounts of `provider`'s CLI (Default last), then *Add or manage accounts…*. The field shows
/// the chosen account with who is signed in.
pub(super) fn choices(ctx: &Ctx, accounts: &Accounts, readings: &PlanReadings, provider: AgentProvider) -> Vec<Choice> {
  let mut choices: Vec<Choice> = accounts
    .of_provider(provider)
    .map(|account| {
      let (detail, warning) = detail(ctx, account, accounts, readings.of_account(account.id()));
      let trigger = account.email().map(|email| {
        ctx.t_args(
          "connections.account.linkValue",
          [("account", name(ctx, account)), ("detail", email.to_owned())],
        )
      });
      Choice {
        value: account.id().to_string(),
        label: name(ctx, account).into(),
        icon: Icon::User,
        detail: Some(detail.into()),
        separator: false,
        warning,
        trigger,
        unavailable: None,
      }
    })
    .collect();
  choices.push(Choice {
    value: MANAGE.to_owned(),
    label: ctx.t("connections.account.manage"),
    icon: Icon::Settings,
    detail: Some(ctx.t("connections.account.manageDetail")),
    separator: true,
    warning: false,
    trigger: None,
    unavailable: None,
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
      icon_in_field: true,
      placeholder: None,
      mono: false,
    }
    .render(ctx)
  }
}

/// The *Account* row of Add connection, after *Program*: the accounts of `provider`'s CLI, `chosen` by id (empty:
/// Default).
pub(super) fn add_row(ctx: &mut Ctx, accounts: &Accounts, provider: AgentProvider, chosen: &Signal<String>) -> Element {
  let readings = crate::services::plan_limits::account_readings(ctx);
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
      width: super::page::CONTROL_WIDTH,
      on_choose: Arc::new(move |chosen| setter.set(chosen)),
    },
  );
  crate::screens::project_settings::kit::form_row(
    crate::ui::form_section::Label::new("connection-account", ctx.t("connections.account.label"))
      .description(ctx.t("connections.account.description")),
    picker,
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

/// The *Account* row of a CLI connection's page (`Account` `mgczX` in `05b`, `M42rM`): the account's name with an arrow
/// that opens it in Settings › Accounts, and in the row's hint who is signed in or what keeps runs from starting on
/// it, with its windows' use as its CLI last reported it. Choosing another account is Edit connection's.
pub(super) fn account_row(
  ctx: &mut Ctx,
  accounts: &Accounts,
  connection: &orchester_core::connection::AgentConnection,
) -> Element {
  let provider = connection.provider();
  let readings = crate::services::plan_limits::account_readings(ctx);
  let account = accounts.of_connection(connection.account(), provider);
  let shown = match account {
    Some(account) => name(ctx, account),
    None => named(ctx, accounts, connection.account(), provider).unwrap_or_default(),
  };
  let detail = account.and_then(|account| short_state(ctx, account).or_else(|| account.email().map(str::to_owned)));
  let limits = account
    .and_then(|account| readings.of_account(account.id()))
    .map(|reading| window_use(ctx, reading))
    .filter(|words| !words.is_empty());
  let hint = match (detail, limits) {
    (Some(detail), Some(limits)) => Some(
      ctx
        .t_args(
          "connections.account.withLimits",
          [("detail", detail), ("limits", limits)],
        )
        .to_string(),
    ),
    (detail, limits) => detail.or(limits),
  };
  let navigation = Navigation::of(ctx);
  let view = account.map_or_else(AccountsView::default, |account| {
    AccountsView::of(account.id(), crate::routes::AccountsPanel::Account)
  });
  let link = crate::ui::pressable::opening(navigation.to(Route::Accounts(view)))
    .id("connection-account-link")
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .align_items(lurq::layout::Alignment::Center)
    .child(crate::screens::project_settings::kit::value(
      "connection-account-name",
      &shown,
    ))
    .child(crate::ui::icon::icon(
      Icon::ArrowUpRight,
      crate::theme::IconSize::Md,
      lurq::app::theme::PaletteColor::TextMuted,
    ));
  let row = crate::screens::project_settings::kit::SettingRow::new(
    "connection-account-row",
    ctx.t("connections.account.label"),
  )
  .info(ctx.t("connections.account.description"))
  .control(link);
  match hint {
    Some(hint) => row.hint(hint),
    None => row,
  }
  .render(ctx)
}

/// The windows of `reading` that have not reset, and how much of each is used: "5-hour 12% · weekly 41%".
fn window_use(ctx: &Ctx, reading: &LimitReading) -> String {
  let now = orchester_core::time::Timestamp::now();
  reading
    .limits
    .windows
    .iter()
    .filter(|window| !window.expired(now))
    .filter_map(|window| {
      let used = window.percent()?;
      let name = match window.minutes() {
        300 => ctx.t("connections.account.window.fiveHour").to_string(),
        10_080 => ctx.t("connections.account.window.weekly").to_string(),
        minutes => ctx
          .t_args(
            "connections.account.window.hours",
            [("hours", (minutes / 60).to_string())],
          )
          .to_string(),
      };
      // A share of at most a few hundred percent: whole ones are what the row shows.
      let used = (used.round() as u64).to_string();
      Some(
        ctx
          .t_args("connections.account.windowUse", [("window", name), ("used", used)])
          .to_string(),
      )
    })
    .collect::<Vec<_>>()
    .join(&*ctx.t("connections.account.separator"))
}
