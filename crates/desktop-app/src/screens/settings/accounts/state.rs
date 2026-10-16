//! What the shown account's state means for runs and what to do (`E96CI`): a sentence, and under it what fixes it —
//! the sign-in and *Check again* (with the login to copy) for an account not signed in or whose sign-in expired,
//! *Choose folder…* and *Remove* for a folder that is gone.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountState, AgentAccount, same_sign_in},
  id::AgentAccountId,
};
use orchester_workspace::accounts::sign_in;

use super::{
  Parts, Work,
  words::{Shown, folder, name, status_command, t, ta},
};
use crate::{
  theme::ExtraTypography,
  ui::{
    button::{ActionButton, ButtonKind},
    command_row::CommandRow,
    icon::Icon,
    message::message_text,
  },
};

/// What the account's state means for runs and what to do, for every state but signed in alone.
pub(super) fn state_sentence(
  ctx: &mut Ctx,
  (parts, work): (&Parts, &Work),
  account: &AgentAccount,
  shown: Shown,
) -> Option<Element> {
  let cli = account.cli();
  let program = cli.program().to_owned();
  let shown_name = name(ctx, account);
  let email = account.email().map(str::to_owned);
  let folder_text = folder(account)
    .map(|folder| folder.display().to_string())
    .unwrap_or_default();
  let version = account
    .last_check()
    .and_then(|check| check.program())
    .map(|checked| checked.version.clone())
    .unwrap_or_default();
  let text = match shown {
    Shown::Checking => ta(ctx, "accounts.state.checking", [("statusCommand", status_command(cli))]),
    Shown::SigningIn => return None,
    Shown::State(state) => match state {
      AccountState::SignedIn => {
        let other = same_sign_in(account, &parts.accounts)?;
        ta(
          ctx,
          "accounts.state.same",
          [("email", email.unwrap_or_default()), ("account", name(ctx, other))],
        )
      }
      AccountState::NotSignedIn => {
        let sentence = ta(
          ctx,
          "accounts.state.notSignedIn",
          [("program", program), ("folder", folder_text)],
        );
        return Some(signed_out(ctx, (parts, work), account, &sentence));
      }
      AccountState::Expired => {
        let sentence = match email {
          Some(email) => ta(
            ctx,
            "accounts.state.expired",
            [("email", email), ("program", program), ("account", shown_name)],
          ),
          None => ta(
            ctx,
            "accounts.state.expiredUnknown",
            [("program", program), ("account", shown_name)],
          ),
        };
        return Some(signed_out(ctx, (parts, work), account, &sentence));
      }
      AccountState::CliMissing => ta(
        ctx,
        "accounts.state.cliMissing",
        [("program", program), ("account", shown_name)],
      ),
      AccountState::Unknown => ta(
        ctx,
        "accounts.state.unknown",
        [("program", program), ("version", version), ("account", shown_name)],
      ),
      AccountState::FolderMissing => {
        let sentence = ta(
          ctx,
          "accounts.state.folderMissing",
          [("folder", folder_text), ("account", shown_name)],
        );
        return Some(folder_missing(ctx, work, account, &sentence));
      }
      AccountState::Unsupported => t(ctx, "accounts.state.unsupported"),
      AccountState::NotChecked => ta(
        ctx,
        "accounts.state.notChecked",
        [("statusCommand", status_command(cli))],
      ),
    },
  };
  Some(paragraph("account-state-sentence", &text).into())
}

/// The sentence of an account not signed in or whose sign-in expired, with the sign-in and *Check again* under it,
/// and, not signed in, the login command to copy (`Not signed in` and `Sign-in expired` in `E96CI`).
fn signed_out(ctx: &mut Ctx, (parts, work): (&Parts, &Work), account: &AgentAccount, sentence: &str) -> Element {
  let never = account.state(orchester_core::connection::invocation::Platform::current()) == AccountState::NotSignedIn;
  let command = never.then(|| {
    let command = sign_in::command(account);
    command_row(ctx, "account-sign-in-command", &command.text)
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(paragraph("account-state-sentence", sentence))
    .child(
      Row::new()
        .id("account-state-actions")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(sign_in_button(ctx, parts, work, account))
        .child(check_again(ctx, parts, work, account.id(), "account-state-check-again")),
    )
    .with_children(command)
    .into()
}

/// *Sign in (again) in a terminal*: unavailable while the CLI is missing or another account's terminal is open; none
/// where Orchester can't sign in yet.
pub(super) fn sign_in_button(ctx: &Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Option<ActionButton> {
  let state = account.state(orchester_core::connection::invocation::Platform::current());
  let key = if matches!(state, AccountState::SignedIn | AccountState::Expired) {
    "accounts.actions.signInAgain"
  } else {
    "accounts.actions.signIn"
  };
  let button = ActionButton::new("account-sign-in", ButtonKind::Secondary, t(ctx, key)).icon(Icon::SquareTerminal);
  let id = account.id();
  match state {
    AccountState::Unsupported => None,
    AccountState::CliMissing => Some(button.unavailable(ta(
      ctx,
      "accounts.signIn.cliMissing",
      [("program", account.cli().program())],
    ))),
    _ if parts.signing_in == Some(id) => Some(button.loading()),
    _ if work.another_signing_in(id) => Some(button.unavailable(t(ctx, "accounts.signIn.another"))),
    _ => {
      let work = work.clone();
      Some(button.on_click(move || work.open_sign_in(id)))
    }
  }
}

/// *Copy folder path*: puts the account's folder on the clipboard.
pub(super) fn copy_folder(ctx: &Ctx, account: &AgentAccount, id: &str) -> ActionButton {
  let text = folder(account).map(|path| path.display().to_string());
  ActionButton::new(
    id.to_owned(),
    ButtonKind::Secondary,
    t(ctx, "accounts.actions.copyFolder"),
  )
  .icon(Icon::Copy)
  .on_click(move || {
    if let Some(text) = &text
      && !lurq::clipboard::copy_to_clipboard(text)
    {
      tracing::warn!("the folder path could not be put on the clipboard");
    }
  })
}

/// *Check again* for `account`, busy while it is checked.
pub(super) fn check_again(ctx: &Ctx, parts: &Parts, work: &Work, account: AgentAccountId, id: &str) -> ActionButton {
  let button = ActionButton::new(
    id.to_owned(),
    ButtonKind::Secondary,
    t(ctx, "accounts.actions.checkAgain"),
  )
  .icon(Icon::RefreshCw);
  if parts.checking.contains(&account) {
    button.loading()
  } else {
    let work = work.clone();
    button.on_click(move || work.check_again(vec![account]))
  }
}

/// The sentence of a folder that is gone, with *Choose folder…* (a folder the person already has, by the rules of a
/// brought folder) and *Remove* (`Folder missing` in `E96CI`).
fn folder_missing(ctx: &mut Ctx, work: &Work, account: &AgentAccount, sentence: &str) -> Element {
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
  let navigation = crate::navigation::Navigation::of(ctx);
  let remove = ActionButton::new(
    "account-folder-missing-remove",
    ButtonKind::Ghost,
    t(ctx, "accounts.actions.remove"),
  )
  .icon(Icon::Trash2)
  .on_click(move || {
    navigation.go(crate::routes::Route::Accounts(crate::routes::AccountsView::of(
      id,
      crate::routes::AccountsPanel::Remove,
    )));
  });
  let error = work
    .change_error()
    .map(|error| crate::ui::field::field_error("account-choose-folder-error", &message_text(ctx, &error)));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(paragraph("account-state-sentence", sentence))
    .child(
      Row::new()
        .id("account-folder-missing-actions")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(choose)
        .child(remove),
    )
    .with_children(error)
    .into()
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
