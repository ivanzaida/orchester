//! Signing in with the CLI's own login (`Sign in · step 2 of Add account, and Sign in again` `UsTGf` in `poD7g`):
//! Orchester opens a terminal window running the CLI's login for the account's folder and waits for it to close, then
//! checks who is signed in; the same command is shown to run in a terminal of one's own. Orchester never sees what is
//! typed there. Step 2 of *Add account* is a page of its own under the tab; an account's page shows the same panel
//! under its head while its terminal is open or after it closed signed out.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountCli, AccountFolder, AccountState, AgentAccount, same_sign_in},
  id::AgentAccountId,
};
use orchester_workspace::{accounts::sign_in::command, display};

use super::{
  Parts, Work, add,
  state::command_row,
  words::{Shown, cli_key, login_command, method_key, name, plan, status_command, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  screens::settings::frame,
  theme::{ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    notice::NoticeTone,
    setting_notice::{SettingNotice, SettingPanel, status_line},
    setting_rows::{SettingGroup, SettingRow},
  },
};

const PANEL: &str = "account-sign-in";

/// Step 2 of *Add account* for the account the route names: step 1 done, and the sign-in.
pub(super) fn page(ctx: &mut Ctx, parts: &Parts, work: &Work) -> Column {
  let pane = add::heading(ctx, frame::pane("account-sign-in-page"));
  let Some(account) = parts.view.account.and_then(|id| parts.account(id)) else {
    return pane;
  };
  let done_key = match account.folder() {
    AccountFolder::Created(_) => "accounts.add.step1Created",
    _ => "accounts.add.step1Brought",
  };
  let done = SettingRow::new(
    "account-add-step-1-done",
    ta(ctx, done_key, [("name", name(ctx, account))]),
  )
  .lead(icon(Icon::CircleCheck, IconSize::Md, PaletteColor::Success))
  .render(ctx);
  pane
    .child(SettingGroup::new("account-add-step-1", t(ctx, "accounts.add.step1")).row(done))
    .child(SettingGroup::new("account-add-step-2", t(ctx, "accounts.add.step2")))
    .child(panel(ctx, (parts, work), account, true))
}

/// Whether an account's page shows the sign-in: while its terminal is open, or after it closed signed out.
pub(super) fn follows(parts: &Parts, work: &Work, account: &AgentAccount) -> bool {
  let id = account.id();
  parts.signing_in == Some(id)
    || (work.opened_at(id).is_some()
      && work.sign_in.state().get().is_fulfilled()
      && parts.shown(account) == Shown::State(AccountState::NotSignedIn))
}

/// What the sign-in came to once the terminal closed: signed in, the CLI missing, or still signed out; `None` while
/// it is open or was not opened yet.
fn outcome(ctx: &mut Ctx, (parts, work): (&Parts, &Work), account: &AgentAccount) -> Option<Element> {
  let id = account.id();
  let cli = account.cli();
  let program = cli.program().to_owned();
  match parts.shown(account) {
    Shown::State(AccountState::SignedIn) => Some(signed_in(ctx, parts, account)),
    Shown::State(AccountState::CliMissing) => Some(
      SettingNotice::new("account-sign-in-cli-missing", NoticeTone::Danger)
        .title(ta(ctx, "accounts.signIn.cliMissing", [("program", program)]))
        .body(ta(
          ctx,
          "accounts.signIn.cliMissingBody",
          [("cli", t(ctx, cli_key(cli)))],
        ))
        .action(check_again(ctx, parts, work, id))
        .into(),
    ),
    Shown::State(_) if work.sign_in.state().get().is_fulfilled() && work.opened_at(id).is_some() => Some(
      SettingNotice::new("account-sign-in-still-out", NoticeTone::Warning)
        .title(ta(
          ctx,
          "accounts.signIn.stillOut",
          [("program", program), ("account", name(ctx, account))],
        ))
        .body(t(ctx, "accounts.signIn.stillOutBody"))
        .part(own_terminal(ctx, account))
        .action(open_button(ctx, work, id, true))
        .action(check_again(ctx, parts, work, id))
        .into(),
    ),
    _ => None,
  }
}

/// Where the sign-in stands: the terminal open, signed in, still signed out, the CLI missing, or not opened yet;
/// `later` adds *Finish later*, which leaves *Add account* for the account's page.
pub(super) fn panel(ctx: &mut Ctx, (parts, work): (&Parts, &Work), account: &AgentAccount, later: bool) -> Element {
  let id = account.id();
  let shown = name(ctx, account);
  let cli = account.cli();
  let program = cli.program().to_owned();
  let error = work
    .sign_in
    .state()
    .get()
    .error
    .map(|error| field_error("account-sign-in-error", &message_text(ctx, &error)));
  if let Some(outcome) = outcome(ctx, (parts, work), account) {
    return Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .with_children(error)
      .child(outcome)
      .into();
  }
  let opened = parts.shown(account) == Shown::SigningIn;
  let text_key = if opened {
    "accounts.signIn.opened"
  } else {
    "accounts.signIn.notOpened"
  };
  let mut panel = SettingPanel::new(PANEL, ta(ctx, "accounts.signIn.title", [("program", program.clone())])).text(
    "account-sign-in-text",
    &ta(
      ctx,
      text_key,
      [("command", login_command(cli)), ("account", shown.clone())],
    ),
  );
  panel = if opened {
    let at = work.opened_at(id).map(display::clock).unwrap_or_default();
    panel.part(status_line(
      "account-sign-in-waiting",
      Icon::LoaderCircle,
      &ta(ctx, "accounts.signIn.waiting", [("time", at)]),
    ))
  } else {
    panel.part(own_terminal(ctx, account))
  };
  panel = panel.note(
    "account-sign-in-privacy",
    &ta(
      ctx,
      "accounts.signIn.privacy",
      [
        ("program", program),
        ("account", shown.clone()),
        ("statusCommand", status_command(cli)),
      ],
    ),
  );
  let open = if parts.signing_in == Some(id) {
    open_button(ctx, work, id, true).loading()
  } else {
    open_button(ctx, work, id, opened)
  };
  let mut actions: Vec<Element> = vec![open.into(), check_again(ctx, parts, work, id).into()];
  if later {
    actions.push(finish_later(ctx, id).into());
    panel = panel.note(
      "account-sign-in-saved",
      &ta(ctx, "accounts.signIn.saved", [("account", shown)]),
    );
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(error)
    .child(panel.actions(actions))
    .into()
}

/// The login to run in a terminal of one's own, with Copy, and what it does there.
fn own_terminal(ctx: &mut Ctx, account: &AgentAccount) -> Element {
  let own = command(account);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(&ta(ctx, "accounts.signIn.ownTerminal", [("shell", own.shell)]))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextSecondary),
    )
    .child(command_row(ctx, "account-sign-in-own", &own.text))
    .child(
      Text::new(&ta(
        ctx,
        "accounts.signIn.ownNote",
        [("program", account.cli().program().to_owned())],
      ))
      .variant(ExtraTypography::Note.role())
      .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// Signed in as the same person as `other`: said plainly, with *Keep both* and *Remove* the new one.
fn same_person(ctx: &mut Ctx, account: &AgentAccount, other: &AgentAccount, email: String) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let shown = name(ctx, account);
  let keep = navigation.clone();
  SettingNotice::new("account-sign-in-same", NoticeTone::Warning)
    .title(ta(
      ctx,
      "accounts.signIn.sameTitle",
      [("email", email), ("account", name(ctx, other))],
    ))
    .body(t(ctx, "accounts.signIn.sameBody"))
    .action(
      ActionButton::new(
        "account-sign-in-keep-both",
        ButtonKind::Secondary,
        t(ctx, "accounts.signIn.keepBoth"),
      )
      .on_click(move || keep.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)))),
    )
    .action(
      ActionButton::new(
        "account-sign-in-remove",
        ButtonKind::DangerGhost,
        ta(ctx, "accounts.signIn.removeIt", [("account", shown)]),
      )
      .on_click(move || navigation.go(Route::Accounts(AccountsView::of(id, AccountsPanel::Remove)))),
    )
    .into()
}

/// Signed in: who, and *Done*; the same person as another account says so and offers to remove the new one.
fn signed_in(ctx: &mut Ctx, parts: &Parts, account: &AgentAccount) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let shown = name(ctx, account);
  let email = account.email().map(str::to_owned);
  if let (Some(other), Some(email)) = (same_sign_in(account, &parts.accounts), email.clone()) {
    return same_person(ctx, account, other, email);
  }
  let checked = account
    .last_check()
    .map(|check| display::clock(check.at))
    .unwrap_or_default();
  let title = email.map_or_else(
    || t(ctx, "accounts.signIn.signedInNoEmail"),
    |email| ta(ctx, "accounts.signIn.signedIn", [("email", email)]),
  );
  let facts = identity_facts(ctx, account);
  let body = if facts.is_empty() {
    ta(
      ctx,
      "accounts.signIn.signedInBody",
      [("time", checked), ("account", shown.clone())],
    )
  } else {
    ta(
      ctx,
      "accounts.signIn.signedInFactsBody",
      [("facts", facts), ("time", checked), ("account", shown.clone())],
    )
  };
  let notice = SettingNotice::new("account-sign-in-done", NoticeTone::Success)
    .title(title)
    .body(body)
    .action(
      ActionButton::new(
        "account-sign-in-finish",
        ButtonKind::Primary,
        t(ctx, "accounts.signIn.done"),
      )
      .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)))),
    );
  // A Claude Code folder is what a person points their own claude at.
  let notice = match (account.cli() == AccountCli::ClaudeCode)
    .then(|| super::words::folder(account))
    .flatten()
  {
    Some(folder) => {
      let path = folder.display().to_string();
      notice.action(
        ActionButton::new(
          "account-sign-in-copy-folder",
          ButtonKind::Secondary,
          t(ctx, "accounts.actions.copyFolder"),
        )
        .icon(Icon::Copy)
        .on_click(move || {
          if !lurq::clipboard::copy_to_clipboard(&path) {
            tracing::warn!("the folder path could not be put on the clipboard");
          }
        }),
      )
    }
    None => notice,
  };
  notice
    .note(ta(
      ctx,
      "accounts.signIn.next",
      [("account", shown), ("cli", t(ctx, cli_key(account.cli())))],
    ))
    .into()
}

/// The plan and how the account signed in, as its CLI reported them (`Max plan · signed in with claude.ai`); empty
/// when it reported neither.
fn identity_facts(ctx: &Ctx, account: &AgentAccount) -> String {
  let identity = account.last_check().and_then(|check| check.known_identity());
  let plan = identity.and_then(|identity| identity.plan.as_deref()).map(plan);
  let method = identity
    .and_then(|identity| identity.method)
    .map(|method| t(ctx, method_key(method)));
  match (plan, method) {
    (Some(plan), Some(method)) => ta(
      ctx,
      "accounts.signIn.planAndMethod",
      [("plan", plan), ("method", method)],
    ),
    (Some(plan), None) => ta(ctx, "accounts.signIn.planFact", [("plan", plan)]),
    (None, Some(method)) => ta(ctx, "accounts.signIn.methodFact", [("method", method)]),
    (None, None) => String::new(),
  }
}

/// *Open the terminal* (*again*): unavailable while another account's terminal is open.
fn open_button(ctx: &Ctx, work: &Work, id: AgentAccountId, again: bool) -> ActionButton {
  let (button_id, key) = if again {
    ("account-sign-in-open-again", "accounts.signIn.openAgain")
  } else {
    ("account-sign-in-open", "accounts.signIn.open")
  };
  let kind = if again {
    ButtonKind::Secondary
  } else {
    ButtonKind::Primary
  };
  let button = ActionButton::new(button_id, kind, t(ctx, key)).icon(Icon::SquareTerminal);
  if work.another_signing_in(id) {
    return button.unavailable(t(ctx, "accounts.signIn.another"));
  }
  let work = work.clone();
  button.on_click(move || work.open_sign_in(id))
}

fn check_again(ctx: &Ctx, parts: &Parts, work: &Work, id: AgentAccountId) -> ActionButton {
  super::state::check_again(ctx, (parts, work), id, ("account-sign-in-check", ButtonKind::Ghost))
}

/// *Finish later*: the account is saved; its page opens.
fn finish_later(ctx: &mut Ctx, id: AgentAccountId) -> ActionButton {
  let navigation = Navigation::of(ctx);
  ActionButton::new(
    "account-sign-in-later",
    ButtonKind::Ghost,
    t(ctx, "accounts.signIn.finishLater"),
  )
  .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account))))
}
