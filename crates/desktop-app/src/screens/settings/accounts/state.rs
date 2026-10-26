//! What an account's state means for runs and what to do, as a notice under its page's head (`Sign-in expired`
//! `M32Lv`, `Folder missing` `pIwz9` in `poD7g`): the state as its title in the state's tone, the sentence today's
//! page says, and what fixes it — the sign-in and *Check again* (with the login to copy) for an account not signed in
//! or whose sign-in expired, *Choose folder…* and *Remove* for a folder that is gone. While Orchester asks the CLI, a
//! line says so. And the actions the page and its menu share: *Check again* and the sign-in.

use lurq::{
  app::ctx::Ctx,
  components::Text,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountState, AgentAccount, same_sign_in},
  connection::invocation::Platform,
  id::AgentAccountId,
};
use orchester_workspace::accounts::sign_in;

use super::{
  Parts, Work,
  words::{Shown, folder, look, name, status_command, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    command_row::CommandRow,
    field::field_error,
    icon::Icon,
    menu::MenuItem,
    message::message_text,
    notice::NoticeTone,
    setting_notice::{SettingNotice, status_line},
  },
};

const ID: &str = "account-state-sentence";

/// What the account's state means for runs and what to do, for every state but signed in alone.
pub(super) fn notice(
  ctx: &mut Ctx,
  (parts, work): (&Parts, &Work),
  account: &AgentAccount,
  shown: Shown,
) -> Option<Element> {
  let cli = account.cli();
  let program = cli.program().to_owned();
  let shown_name = name(ctx, account);
  let folder_text = folder(account)
    .map(|folder| folder.display().to_string())
    .unwrap_or_default();
  let state = match shown {
    Shown::Checking => {
      let text = ta(ctx, "accounts.state.checking", [("statusCommand", status_command(cli))]);
      return Some(status_line(ID, Icon::LoaderCircle, &text).into());
    }
    Shown::SigningIn => return None,
    Shown::State(state) => state,
  };
  let titled = |tone| SettingNotice::new(ID, tone).title(t(ctx, look(shown).key));
  let notice = match state {
    AccountState::SignedIn => {
      let other = same_sign_in(account, &parts.accounts)?;
      let email = account.email().map(str::to_owned).unwrap_or_default();
      SettingNotice::new(ID, NoticeTone::Warning).body(ta(
        ctx,
        "accounts.state.same",
        [("email", email), ("account", name(ctx, other))],
      ))
    }
    AccountState::NotSignedIn => {
      let body = ta(
        ctx,
        "accounts.state.notSignedIn",
        [("program", program), ("folder", folder_text)],
      );
      return Some(signed_out(
        ctx,
        (parts, work),
        account,
        titled(NoticeTone::Warning).body(body),
      ));
    }
    AccountState::Expired => {
      let body = expired(ctx, account, (program, shown_name));
      return Some(signed_out(
        ctx,
        (parts, work),
        account,
        titled(NoticeTone::Warning).body(body),
      ));
    }
    AccountState::CliMissing => titled(NoticeTone::Danger).body(ta(
      ctx,
      "accounts.state.cliMissing",
      [("program", program), ("account", shown_name)],
    )),
    AccountState::Unknown => {
      let version = account
        .last_check()
        .and_then(|check| check.program())
        .map(|checked| checked.version.clone())
        .unwrap_or_default();
      titled(NoticeTone::Warning).body(ta(
        ctx,
        "accounts.state.unknown",
        [("program", program), ("version", version), ("account", shown_name)],
      ))
    }
    AccountState::FolderMissing => {
      let body = ta(
        ctx,
        "accounts.state.folderMissing",
        [("folder", folder_text), ("account", shown_name)],
      );
      return Some(folder_missing(
        ctx,
        work,
        account,
        titled(NoticeTone::Danger).body(body),
      ));
    }
    AccountState::Unsupported => titled(NoticeTone::Info).body(t(ctx, "accounts.state.unsupported")),
    AccountState::NotChecked => titled(NoticeTone::Info).body(ta(
      ctx,
      "accounts.state.notChecked",
      [("statusCommand", status_command(cli))],
    )),
  };
  Some(notice.into())
}

/// What an expired sign-in means, with who was signed in when the CLI said.
fn expired(ctx: &Ctx, account: &AgentAccount, (program, shown): (String, String)) -> String {
  match account.email() {
    Some(email) => ta(
      ctx,
      "accounts.state.expired",
      [("email", email.to_owned()), ("program", program), ("account", shown)],
    ),
    None => ta(
      ctx,
      "accounts.state.expiredUnknown",
      [("program", program), ("account", shown)],
    ),
  }
}

/// An account not signed in or whose sign-in expired: the sign-in and *Check again*, and, never signed in, the login
/// command to copy.
fn signed_out(ctx: &mut Ctx, (parts, work): (&Parts, &Work), account: &AgentAccount, notice: SettingNotice) -> Element {
  let never = account.state(Platform::current()) == AccountState::NotSignedIn;
  let notice = match never.then(|| command_row(ctx, "account-sign-in-command", &sign_in::command(account).text)) {
    Some(command) => notice.part(command),
    None => notice,
  };
  let notice = match sign_in_button(ctx, parts, work, account) {
    Some(button) => notice.action(button),
    None => notice,
  };
  notice
    .action(check_again(
      ctx,
      (parts, work),
      account.id(),
      ("account-state-check-again", ButtonKind::Ghost),
    ))
    .into()
}

/// The sign-in's words: *Sign in again in a terminal* for an account that was signed in.
fn sign_in_key(account: &AgentAccount) -> &'static str {
  let state = account.state(Platform::current());
  if matches!(state, AccountState::SignedIn | AccountState::Expired) {
    "accounts.actions.signInAgain"
  } else {
    "accounts.actions.signIn"
  }
}

/// Why the sign-in can't open now: without the CLI, or while another account's terminal is open; `None` where
/// Orchester can't sign in yet.
fn sign_in_blocked(ctx: &Ctx, work: &Work, account: &AgentAccount) -> Option<Option<String>> {
  match account.state(Platform::current()) {
    AccountState::Unsupported => None,
    AccountState::CliMissing => Some(Some(ta(
      ctx,
      "accounts.signIn.cliMissing",
      [("program", account.cli().program())],
    ))),
    _ if work.another_signing_in(account.id()) => Some(Some(t(ctx, "accounts.signIn.another"))),
    _ => Some(None),
  }
}

/// *Sign in (again) in a terminal*: unavailable while the CLI is missing or another account's terminal is open; none
/// where Orchester can't sign in yet.
pub(super) fn sign_in_button(ctx: &Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Option<ActionButton> {
  let button =
    ActionButton::new("account-sign-in", ButtonKind::Primary, t(ctx, sign_in_key(account))).icon(Icon::SquareTerminal);
  let id = account.id();
  match sign_in_blocked(ctx, work, account)? {
    Some(reason) => Some(button.unavailable(reason)),
    None if parts.signing_in == Some(id) => Some(button.loading()),
    None => {
      let work = work.clone();
      Some(button.on_click(move || work.open_sign_in(id)))
    }
  }
}

/// The sign-in as a row of the page's *…* menu.
pub(super) fn sign_in_item(ctx: &Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Option<MenuItem> {
  let label = t(ctx, sign_in_key(account));
  let id = account.id();
  let item = match sign_in_blocked(ctx, work, account)? {
    Some(reason) => MenuItem::unavailable("account-sign-in-item", label, reason),
    None if parts.signing_in == Some(id) => {
      MenuItem::unavailable("account-sign-in-item", label, t(ctx, "accounts.status.signingIn"))
    }
    None => {
      let work = work.clone();
      MenuItem::new("account-sign-in-item", label, move || work.open_sign_in(id))
    }
  };
  Some(item.icon(Icon::SquareTerminal))
}

/// *Check again* for `account` as a button of `kind`, busy while it is checked.
pub(super) fn check_again(
  ctx: &Ctx,
  (parts, work): (&Parts, &Work),
  account: AgentAccountId,
  (id, kind): (&str, ButtonKind),
) -> ActionButton {
  let button = ActionButton::new(id.to_owned(), kind, t(ctx, "accounts.actions.checkAgain")).icon(Icon::RefreshCw);
  if parts.checking.contains(&account) {
    button.loading()
  } else {
    let work = work.clone();
    button.on_click(move || work.check_again(vec![account]))
  }
}

/// A folder that is gone: *Choose folder…* (a folder the person already has, by the rules of a brought folder) and
/// *Remove*.
fn folder_missing(ctx: &mut Ctx, work: &Work, account: &AgentAccount, notice: SettingNotice) -> Element {
  let id = account.id();
  let choose = ActionButton::new(
    "account-choose-folder",
    ButtonKind::Secondary,
    t(ctx, "accounts.actions.chooseFolder"),
  )
  .icon(Icon::FolderOpen);
  let choose = if work.choose.state().get().is_pending() || work.change.state().get().is_pending() {
    choose.loading()
  } else {
    let pick = work.choose.clone();
    choose.on_click(move || pick.run(id))
  };
  let navigation = Navigation::of(ctx);
  let remove = ActionButton::new(
    "account-folder-missing-remove",
    ButtonKind::DangerGhost,
    t(ctx, "accounts.actions.remove"),
  )
  .icon(Icon::Trash2)
  .on_click(move || navigation.go(Route::Accounts(AccountsView::of(id, AccountsPanel::Remove))));
  let notice = notice.action(choose).action(remove);
  match work.change_error() {
    Some(error) => notice
      .part(field_error("account-choose-folder-error", &message_text(ctx, &error)))
      .into(),
    None => notice.into(),
  }
}

/// A command in its box with Copy (`Command row` `PEnj5`); Copy puts it on the clipboard.
pub(super) fn command_row(ctx: &mut Ctx, id: &str, text: &str) -> Element {
  let copied = text.to_owned();
  CommandRow::new(id.to_owned(), text.to_owned(), t(ctx, "common.copy"), move || {
    lurq::clipboard::copy_to_clipboard(&copied)
  })
  .prompt()
  .render(ctx)
}

pub(super) fn paragraph(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Prose.role())
    .width(Dimension::Pct(100.0))
}
