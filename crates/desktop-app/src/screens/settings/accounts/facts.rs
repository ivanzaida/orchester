//! An account's *Sign-in* group (`Sign-in` `m6jRJ` in `03b · Account`): who is signed in and the plan, the folder
//! with Copy and *Open folder* (what runs on the account get in its tooltip), how it signed in, the program that
//! checked it and when; under the group, that Orchester never reads a token. A fact the CLI does not report says so in
//! `text_muted`.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  clipboard::copy_to_clipboard,
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  account::{AccountCheckResult, AccountState, AgentAccount},
  connection::invocation::Platform,
  time::Timestamp,
};
use orchester_workspace::display;

use super::{
  details::signed_in,
  words::{folder, method_key, plan, status_command, t, ta},
};
use crate::{
  theme::ExtraSpacing,
  ui::{
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
    setting_rows::{SettingGroup, SettingRow, mono_value, value},
  },
};

pub(super) fn group(ctx: &mut Ctx, account: &AgentAccount) -> Element {
  let check = account.last_check();
  let identity = check.and_then(|check| check.known_identity());
  let program = account.cli().program();
  let not_reported = ta(ctx, "accounts.details.notReported", [("program", program)]);
  let mut rows = Vec::new();
  if signed_in(account) {
    let email = identity.and_then(|identity| identity.email.clone());
    let plan_name = identity.and_then(|identity| identity.plan.as_deref()).map(plan);
    let who = match (email, plan_name) {
      (Some(email), Some(plan)) => Ok(ta(ctx, "accounts.identity.emailPlan", [("email", email), ("plan", plan)])),
      (Some(email), None) => Ok(email),
      (None, Some(plan)) => Ok(ta(ctx, "accounts.identity.plan", [("plan", plan)])),
      (None, None) => Err(not_reported.clone()),
    };
    rows.push(fact(ctx, ("account-fact-email", "accounts.details.signedInAs"), who));
  }
  rows.push(folder_row(ctx, account));
  if signed_in(account) {
    let method = identity
      .and_then(|identity| identity.method)
      .map(|method| t(ctx, method_key(method)))
      .ok_or(not_reported);
    rows.push(fact(ctx, ("account-fact-method", "accounts.details.signedInWith"), method));
  }
  let checked_with = checked_with(ctx, account);
  rows.push(fact(
    ctx,
    ("account-fact-checked-with", "accounts.details.checkedWith"),
    Ok(checked_with),
  ));
  let last_checked = check.map_or_else(
    || t(ctx, "accounts.details.neverChecked"),
    |check| message_text(ctx, &display::elapsed(Timestamp::now(), check.at)).to_string(),
  );
  rows.push(fact(
    ctx,
    ("account-fact-last-checked", "accounts.details.lastChecked"),
    Ok(last_checked),
  ));
  let note = ta(
    ctx,
    "accounts.noToken",
    [
      ("program", program.to_owned()),
      ("statusCommand", status_command(account.cli())),
    ],
  );
  SettingGroup::new("account-facts", t(ctx, "accounts.details.signIn"))
    .rows(rows)
    .note(note)
    .into()
}

/// A fact's row, its value at the end: muted when the CLI does not report it.
fn fact(ctx: &mut Ctx, (id, key): (&str, &str), shown: Result<String, String>) -> Element {
  let control = match &shown {
    Ok(text) => value(&format!("{id}-value"), text),
    Err(missing) => value(&format!("{id}-value"), missing).color(PaletteColor::TextMuted),
  };
  SettingRow::new(id.to_owned(), t(ctx, key)).control(control).render(ctx)
}

/// *Folder*: where the CLI keeps the sign-in, with Copy and *Open folder* while it is there; what runs on the account
/// get in its tooltip.
fn folder_row(ctx: &mut Ctx, account: &AgentAccount) -> Element {
  let path = folder(account);
  let text = path
    .as_ref()
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let text = if account.is_default() {
    ta(ctx, "accounts.details.ownFolder", [("folder", text)])
  } else {
    text
  };
  let gone = account.state(Platform::current()) == AccountState::FolderMissing;
  let buttons = if gone {
    Vec::new()
  } else {
    folder_buttons(ctx, path.as_ref())
  };
  let control = Row::new()
    .id("account-fact-folder-value")
    .flex_shrink(1.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(mono_value("account-fact-folder-path", &text))
    .with_children(buttons);
  SettingRow::new("account-fact-folder", t(ctx, "accounts.details.folder"))
    .hint(t(ctx, "accounts.details.folderHint"))
    .info(runs_get(ctx, account))
    .control(control)
    .render(ctx)
}

/// Copy and *Open folder* for the folder at `path`.
fn folder_buttons(ctx: &mut Ctx, path: Option<&std::path::PathBuf>) -> Vec<Element> {
  let copied = path.map(|path| path.display().to_string());
  let copy = IconButton::new("account-copy-folder", Icon::Copy, t(ctx, "accounts.actions.copyFolder"))
    .small()
    .on_click(move || {
      if let Some(copied) = &copied
        && !copy_to_clipboard(copied)
      {
        tracing::warn!("the folder path could not be put on the clipboard");
      }
    })
    .render(ctx);
  let opened = path.cloned();
  let open = IconButton::new("account-open-folder", Icon::FolderOpen, t(ctx, "accounts.actions.openFolder"))
    .small()
    .on_click(move || {
      if let Some(path) = &opened {
        open(path.clone());
      }
    })
    .render(ctx);
  vec![copy, open]
}

/// The program the last check ran, or why it ran none: no CLI on the PATH, or no folder to check.
fn checked_with(ctx: &Ctx, account: &AgentAccount) -> String {
  let program = account.cli().program();
  match account.last_check() {
    None => t(ctx, "accounts.details.neverChecked"),
    Some(check) => match (check.program(), &check.result) {
      (Some(checked), _) => ta(
        ctx,
        "accounts.details.checkedWithValue",
        [("program", program.to_owned()), ("version", checked.version.clone())],
      ),
      (None, AccountCheckResult::CliMissing { .. }) => {
        ta(ctx, "accounts.details.checkedWithNoCli", [("program", program)])
      }
      (None, _) => t(ctx, "accounts.details.checkedWithNoFolder"),
    },
  }
}

/// What a run on the account gets (`RXAmO`).
fn runs_get(ctx: &Ctx, account: &AgentAccount) -> String {
  let cli = account.cli();
  let program = cli.program().to_owned();
  let variable = cli.folder_variable().to_owned();
  let own = orchester_workspace::accounts::own_folder(cli)
    .map(|folder| folder.display().to_string())
    .unwrap_or_default();
  if account.is_default() {
    ta(
      ctx,
      "accounts.runsGet.default",
      [("program", program), ("variable", variable), ("folder", own)],
    )
  } else {
    ta(
      ctx,
      "accounts.runsGet.text",
      [("program", program), ("variable", variable), ("defaultFolder", own)],
    )
  }
}

/// Opens `folder` in the system's file manager, off the UI thread: the launcher can take a moment.
fn open(folder: std::path::PathBuf) {
  let opening = std::thread::Builder::new()
    .name("open-account-folder".to_owned())
    .spawn(move || {
      if let Err(error) = orchester_exec::external::open_path(&folder) {
        tracing::warn!(%error, "cannot open an account's folder");
      }
    });
  if let Err(error) = opening {
    tracing::error!(%error, "cannot start opening an account's folder");
  }
}
