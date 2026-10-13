//! How an account reads in Settings › Accounts (`E96CI`): its name, its state as an icon and a word in the state's
//! color, the line under its name, its folder, and the CLI's words for it.

use std::path::PathBuf;

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::account::{AccountCli, AccountIdentity, AccountState, AgentAccount, SignInMethod, same_sign_in};
use orchester_workspace::{accounts::own_folder, display::shorten_path};

use crate::ui::icon::Icon;

/// How long a folder may read in a list row before its middle is left out (`q8bz5`).
const ROW_FOLDER_CHARS: usize = 40;

/// What a row and the details header show for a state: the icon, its color, the word and the word's color.
pub(super) struct Look {
  pub icon: Icon,
  pub icon_color: PaletteColor,
  pub key: &'static str,
  pub color: PaletteColor,
}

/// The shown state of an account: its own, or *Checking…* while Orchester asks its CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shown {
  State(AccountState),
  Checking,
  /// *Signing in…* while its sign-in terminal is open.
  SigningIn,
}

pub(super) fn look(shown: Shown) -> Look {
  let look = |icon, icon_color, key, color| Look {
    icon,
    icon_color,
    key,
    color,
  };
  match shown {
    Shown::Checking => look(
      Icon::LoaderCircle,
      PaletteColor::TextMuted,
      "accounts.status.checking",
      PaletteColor::TextSecondary,
    ),
    Shown::SigningIn => look(
      Icon::LoaderCircle,
      PaletteColor::TextMuted,
      "accounts.status.signingIn",
      PaletteColor::TextSecondary,
    ),
    Shown::State(state) => match state {
      AccountState::SignedIn => look(
        Icon::CircleCheck,
        PaletteColor::Success,
        "accounts.status.signedIn",
        PaletteColor::TextSecondary,
      ),
      AccountState::NotSignedIn => look(
        Icon::CircleAlert,
        PaletteColor::Warning,
        "accounts.status.notSignedIn",
        PaletteColor::Warning,
      ),
      AccountState::Expired => look(
        Icon::CircleAlert,
        PaletteColor::Warning,
        "accounts.status.expired",
        PaletteColor::Warning,
      ),
      AccountState::Unknown => look(
        Icon::CircleHelp,
        PaletteColor::TextMuted,
        "accounts.status.unknown",
        PaletteColor::TextSecondary,
      ),
      AccountState::CliMissing => look(
        Icon::CircleX,
        PaletteColor::Danger,
        "accounts.status.cliMissing",
        PaletteColor::Danger,
      ),
      AccountState::FolderMissing => look(
        Icon::FolderX,
        PaletteColor::Danger,
        "accounts.status.folderMissing",
        PaletteColor::Danger,
      ),
      AccountState::Unsupported => look(
        Icon::Ban,
        PaletteColor::TextMuted,
        "accounts.status.unsupported",
        PaletteColor::TextSecondary,
      ),
      AccountState::NotChecked => look(
        Icon::CircleDashed,
        PaletteColor::TextMuted,
        "accounts.status.notChecked",
        PaletteColor::TextMuted,
      ),
    },
  }
}

/// The account's name as people see it: the one given, or the translated *Default*.
pub(super) fn name(ctx: &Ctx, account: &AgentAccount) -> String {
  account
    .name()
    .map_or_else(|| t(ctx, "accounts.default.name"), |name| name.as_str().to_owned())
}

pub(super) fn cli_key(cli: AccountCli) -> &'static str {
  match cli {
    AccountCli::ClaudeCode => "accounts.cli.claudeCode",
    AccountCli::Codex => "accounts.cli.codex",
  }
}

/// The plan as the CLI names it (`max`), as a word (`Max`).
pub(super) fn plan(plan: &str) -> String {
  crate::services::accounts::plan_name(plan)
}

/// The words for how the CLI signed in.
pub(super) const fn method_key(method: SignInMethod) -> &'static str {
  match method {
    SignInMethod::ClaudeAi => "accounts.method.claudeAi",
    SignInMethod::Console => "accounts.method.console",
    SignInMethod::ChatGpt => "accounts.method.chatGpt",
    SignInMethod::ApiKey => "accounts.method.apiKey",
  }
}

/// The CLI's status command as the person would type it.
pub(super) fn status_command(cli: AccountCli) -> String {
  std::iter::once(cli.program())
    .chain(cli.status_args().iter().copied())
    .collect::<Vec<_>>()
    .join(" ")
}

/// The CLI's login as the person would type it.
pub(super) fn login_command(cli: AccountCli) -> String {
  std::iter::once(cli.program())
    .chain(cli.login_args().iter().copied())
    .collect::<Vec<_>>()
    .join(" ")
}

/// The folder the account's CLI uses: its own, or Default's as the environment names it.
pub(super) fn folder(account: &AgentAccount) -> Option<PathBuf> {
  account
    .folder()
    .path()
    .map(PathBuf::from)
    .or_else(|| own_folder(account.cli()))
}

/// The CLI's own folder, Default's, as the environment names it.
pub(super) fn own_folder_text(cli: AccountCli) -> String {
  own_folder(cli).map_or_else(String::new, |folder| folder.display().to_string())
}

/// The folder, short enough for a list row.
pub(super) fn row_folder(account: &AgentAccount) -> String {
  folder(account).map_or_else(String::new, |folder| shorten_path(&folder, ROW_FOLDER_CHARS))
}

/// The line under an account's name: who is signed in and the plan, or what to do.
pub(super) fn identity(ctx: &Ctx, account: &AgentAccount, accounts: &[AgentAccount], shown: Shown) -> String {
  let known = account.last_check().and_then(|check| check.known_identity());
  let email = known.and_then(|identity| identity.email.clone());
  match shown {
    Shown::SigningIn => t(ctx, "accounts.identity.waiting"),
    Shown::Checking => known.map_or_else(
      || t(ctx, "accounts.identity.notChecked"),
      |identity| signed_in(ctx, identity),
    ),
    Shown::State(state) => match state {
      AccountState::SignedIn => match (same_sign_in(account, accounts), email) {
        (Some(other), Some(email)) => ta(
          ctx,
          "accounts.identity.same",
          [("email", email), ("account", name(ctx, other))],
        ),
        _ => known.map_or_else(
          || t(ctx, "accounts.identity.signedIn"),
          |identity| signed_in(ctx, identity),
        ),
      },
      AccountState::NotSignedIn | AccountState::NotChecked if account.is_default() => {
        t(ctx, "accounts.default.identity")
      }
      AccountState::NotSignedIn => t(ctx, "accounts.identity.notSignedIn"),
      AccountState::Expired => email.map_or_else(
        || t(ctx, "accounts.identity.wasUnknown"),
        |email| ta(ctx, "accounts.identity.was", [("email", email)]),
      ),
      AccountState::CliMissing => email.map_or_else(
        || t(ctx, "accounts.identity.noCli"),
        |email| ta(ctx, "accounts.identity.lastKnown", [("email", email)]),
      ),
      AccountState::Unknown => email.map_or_else(
        || t(ctx, "accounts.identity.unread"),
        |email| ta(ctx, "accounts.identity.whenLastRead", [("email", email)]),
      ),
      AccountState::FolderMissing => email.map_or_else(
        || t(ctx, "accounts.identity.folderGone"),
        |email| ta(ctx, "accounts.identity.whenLastRead", [("email", email)]),
      ),
      AccountState::Unsupported => ta(
        ctx,
        "accounts.identity.unsupported",
        [("cli", t(ctx, cli_key(account.cli())))],
      ),
      AccountState::NotChecked => t(ctx, "accounts.identity.notChecked"),
    },
  }
}

/// Who is signed in and the plan, as far as the CLI says.
fn signed_in(ctx: &Ctx, identity: &AccountIdentity) -> String {
  match (&identity.email, &identity.plan) {
    (Some(email), Some(plan_name)) => ta(
      ctx,
      "accounts.identity.emailPlan",
      [("email", email.clone()), ("plan", plan(plan_name))],
    ),
    (Some(email), None) => ta(ctx, "accounts.identity.email", [("email", email.clone())]),
    (None, Some(plan_name)) => ta(ctx, "accounts.identity.plan", [("plan", plan(plan_name))]),
    (None, None) => t(ctx, "accounts.identity.signedIn"),
  }
}

/// The words of `key`, as owned text.
pub(super) fn t(ctx: &Ctx, key: &str) -> String {
  ctx.t(key).to_string()
}

/// The words of `key` with `args` in their places, as owned text.
pub(super) fn ta<V: ToString, const N: usize>(ctx: &Ctx, key: &str, args: [(&str, V); N]) -> String {
  ctx.t_args(key, args).to_string()
}
