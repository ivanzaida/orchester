//! The *Account* tab of the shown account (`Detail body` `pwIrF` in `t098v`): what its state means for runs and what
//! to do (`E96CI`), who is signed in, the plan, the folder and the program that checked it, the folder's actions,
//! its plan limits (`Plan window row` `B9myCE`) with a Claude Code account's *Check now*, what runs on it get, and
//! that Orchester never reads a token.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountCheckResult, AccountCli, AccountState, AgentAccount},
  connection::plan_limits::{LimitReading, LimitWindow, ReadingSource},
  time::Timestamp,
};
use orchester_workspace::display;

use super::{
  Parts, PlanCheck, Work,
  state::{copy_folder, paragraph, sign_in_button, state_sentence},
  words::{Shown, folder, method_key, plan, status_command, t, ta},
};
use crate::{
  services::plan_limits,
  theme::{AccountsSpacing, ExtraTypography, SettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    inspector::{self, PropertyValue, property},
    message::message_text,
    plan_window::{PlanWindow, plan_window, share, when},
  },
};

/// A five-hour window and a weekly one, in minutes.
const FIVE_HOURS: u64 = 300;
const WEEK: u64 = 10_080;

pub(super) fn body(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let shown = parts.shown(account);
  let reading = last_reading(ctx, account);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(AccountsSpacing::Parts.role())
    .with_children(state_sentence(ctx, (parts, work), account, shown))
    .child(facts(ctx, account))
    // A folder that is gone can't be opened or signed in to: *Choose folder…* and *Remove* are under its sentence.
    .with_children(
      (shown != Shown::State(AccountState::FolderMissing)).then(|| folder_actions(ctx, parts, work, account)),
    )
    .with_children(
      (shown != Shown::State(AccountState::Unsupported)).then(|| limits(ctx, work, account, reading.as_ref())),
    )
    .child(runs_get(ctx, account))
    .child(inspector::note(
      "account-no-token",
      Icon::KeyRound,
      &ta(
        ctx,
        "accounts.noToken",
        [
          ("program", account.cli().program().to_owned()),
          ("statusCommand", status_command(account.cli())),
        ],
      ),
    ))
    .into()
}

/// The last reading kept of the account's plan limits, with where it came from, as read.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored(Option<(LimitReading, ReadingSource)>);

impl lurq::app::component::DevtoolsInspectable for Stored {}

/// The account's last plan reading and where it came from: this session's, or the last one kept (a project's turn or
/// a read), whichever is newer.
fn last_reading(ctx: &mut Ctx, account: &AgentAccount) -> Option<(LimitReading, ReadingSource)> {
  let id = account.id();
  let readings = plan_limits::readings(ctx);
  let session = readings.of_account(id).cloned().zip(readings.source_of(id));
  let workspace = crate::services::Services::of(ctx).workspace;
  let stored = ctx
    .future(id, move |id| {
      let workspace = workspace.clone();
      async move {
        orchester_workspace::background::run(move || workspace.last_plan_reading(id))
          .await?
          .map(Stored)
          .map_err(|error| error.message())
      }
    })
    .state()
    .get()
    .data
    .and_then(|stored| stored.0);
  match (session, stored) {
    (Some(session), Some(stored)) if stored.0.at > session.0.at => Some(stored),
    (Some(session), _) => Some(session),
    (None, stored) => stored,
  }
}

/// Who is signed in, the plan, how, the folder, the program that checked and when (`U4FViO`).
fn facts(ctx: &Ctx, account: &AgentAccount) -> Element {
  let check = account.last_check();
  let identity = check.and_then(|check| check.known_identity());
  let program = account.cli().program();
  let not_reported = || ta(ctx, "accounts.details.notReported", [("program", program)]);
  let signed_in = account.state(orchester_core::connection::invocation::Platform::current()) == AccountState::SignedIn;
  let mut rows = Column::new().id("account-facts-rows").width(Dimension::Pct(100.0));
  if signed_in {
    let email = identity.and_then(|identity| identity.email.clone());
    let plan_text = identity.and_then(|identity| identity.plan.as_deref()).map(|plan_name| {
      ta(
        ctx,
        "accounts.details.planValue",
        [("plan", plan(plan_name)), ("program", program.to_owned())],
      )
    });
    let method = identity
      .and_then(|identity| identity.method)
      .map(|method| t(ctx, method_key(method)));
    rows = rows
      .child(fact(
        ctx,
        "account-fact-email",
        "accounts.details.signedInAs",
        email.ok_or_else(not_reported),
      ))
      .child(fact(
        ctx,
        "account-fact-plan",
        "accounts.details.plan",
        plan_text.ok_or_else(not_reported),
      ))
      .child(fact(
        ctx,
        "account-fact-method",
        "accounts.details.signedInWith",
        method.ok_or_else(not_reported),
      ));
  }
  let folder_text = folder(account)
    .map(|path| path.display().to_string())
    .unwrap_or_default();
  let folder_text = if account.is_default() {
    ta(ctx, "accounts.details.ownFolder", [("folder", folder_text)])
  } else {
    folder_text
  };
  let checked_with = checked_with(ctx, account);
  let last_checked = check.map_or_else(
    || t(ctx, "accounts.details.neverChecked"),
    |check| message_text(ctx, &display::elapsed(Timestamp::now(), check.at)).to_string(),
  );
  rows
    .child(property(
      "account-fact-folder",
      &t(ctx, "accounts.details.folder"),
      &folder_text,
      PropertyValue::Mono,
    ))
    .child(property(
      "account-fact-checked-with",
      &t(ctx, "accounts.details.checkedWith"),
      &checked_with,
      PropertyValue::Plain,
    ))
    .child(property(
      "account-fact-last-checked",
      &t(ctx, "accounts.details.lastChecked"),
      &last_checked,
      PropertyValue::Plain,
    ))
    .into()
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

/// A fact the CLI may not report: muted when it does not.
fn fact(ctx: &Ctx, id: &str, key: &str, value: Result<String, String>) -> Row {
  match value {
    Ok(value) => property(id, &t(ctx, key), &value, PropertyValue::Plain),
    Err(missing) => property(id, &t(ctx, key), &missing, PropertyValue::Muted),
  }
}

/// *Copy folder path*, *Open folder* and *Sign in (again) in a terminal* (`mn3FG`).
fn folder_actions(ctx: &Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let path = folder(account);
  let copy = copy_folder(ctx, account, "account-copy-folder");
  let open = {
    ActionButton::new(
      "account-open-folder",
      ButtonKind::Secondary,
      t(ctx, "accounts.actions.openFolder"),
    )
    .icon(Icon::FolderOpen)
    .on_click(move || {
      if let Some(path) = &path {
        open(path.clone());
      }
    })
  };
  // Not signed in and expired have the sign-in under their sentence (`E96CI`).
  let state = account.state(orchester_core::connection::invocation::Platform::current());
  let sign_in = (!matches!(state, AccountState::NotSignedIn | AccountState::Expired))
    .then(|| sign_in_button(ctx, parts, work, account))
    .flatten();
  Row::new()
    .id("account-folder-actions")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(copy)
    .child(open)
    .with_children(sign_in)
    .into()
}

/// The account's plan windows as its CLI last reported them (`R7GCQ`), with when and how they were read; a Claude Code
/// account's *Check now* and its error, if it failed (not drawn in the design yet).
fn limits(ctx: &Ctx, work: &Work, account: &AgentAccount, reading: Option<&(LimitReading, ReadingSource)>) -> Element {
  let now = Timestamp::now();
  let program = account.cli().program();
  let mut section = Column::new()
    .id("account-plan-limits")
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .child(limits_head(ctx, work, account, reading, now));
  if let PlanCheck::Failed(error) = work.plan_check_of(account.id()) {
    section = section.child(field_error("account-plan-check-error", &message_text(ctx, &error)));
  }
  let reading = reading.map(|(reading, _)| reading);
  let windows: Vec<&LimitWindow> = reading
    .map(|reading| {
      reading
        .limits
        .windows
        .iter()
        .filter(|window| !window.expired(now))
        .collect()
    })
    .unwrap_or_default();
  if windows.is_empty() {
    let key = match account.cli() {
      AccountCli::ClaudeCode => "accounts.planLimits.none",
      AccountCli::Codex => "accounts.planLimits.noneCodex",
    };
    section = section.child(note("account-plan-none", &ta(ctx, key, [("program", program)])));
  }
  for window in windows {
    let reading = reading.expect("windows come from a reading");
    section = section.child(window_row(ctx, window, reading, now));
  }
  let note_key = match account.cli() {
    AccountCli::ClaudeCode => "accounts.planLimits.noteClaude",
    AccountCli::Codex => "accounts.planLimits.noteCodex",
  };
  section.child(note("account-plan-note", &t(ctx, note_key))).into()
}

/// *Plan limits*, when the reading is from and whether a turn reported it or Orchester read it, and a Claude Code
/// account's *Check now*.
fn limits_head(
  ctx: &Ctx,
  work: &Work,
  account: &AgentAccount,
  reading: Option<&(LimitReading, ReadingSource)>,
  now: Timestamp,
) -> Row {
  let meta = reading.map(|(reading, source)| {
    let at = when(ctx, reading.at, now, ("accounts.time.today", "accounts.time.day"));
    let key = match source {
      ReadingSource::Turn => "accounts.planLimits.metaTurn",
      ReadingSource::Read => "accounts.planLimits.metaRead",
    };
    ta(ctx, key, [("time", at)])
  });
  let check = (account.cli() == AccountCli::ClaudeCode).then(|| {
    let button = ActionButton::new(
      "account-plan-check",
      ButtonKind::Secondary,
      t(ctx, "accounts.planLimits.checkNow"),
    )
    .icon(Icon::RefreshCw);
    if work.plan_check_of(account.id()) == PlanCheck::Running {
      button.loading()
    } else {
      let (work, id) = (work.clone(), account.id());
      button.on_click(move || work.check_plan(id))
    }
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&t(ctx, "accounts.planLimits.title")).variant(SettingsText::SectionTitle.role()))
    .child(Spacer::new().flex(1.0))
    .with_children(meta.map(|meta| {
      Text::new(&meta)
        .id("account-plan-meta")
        .variant(ExtraTypography::Note.role())
    }))
    .with_children(check)
}

fn note(id: &str, text: &str) -> Text {
  Text::new(text)
    .id(id.to_owned())
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0))
}

/// One plan window: its name, a meter, the share used and when it resets (`B9myCE`).
fn window_row(ctx: &Ctx, window: &LimitWindow, reading: &LimitReading, now: Timestamp) -> Element {
  let name = match window.minutes() {
    FIVE_HOURS => t(ctx, "accounts.planLimits.fiveHour"),
    WEEK => t(ctx, "accounts.planLimits.weekly"),
    minutes => ta(
      ctx,
      "accounts.planLimits.window",
      [("hours", (minutes / 60).to_string())],
    ),
  };
  let (used, reached) = share(window, reading, now);
  let resets = window
    .resets()
    .map(|resets| when(ctx, resets, now, ("accounts.time.today", "accounts.time.day")));
  let resets = match (reached, resets) {
    (true, Some(time)) => ta(ctx, "accounts.planLimits.reached", [("time", time)]),
    (false, Some(time)) => ta(ctx, "accounts.planLimits.resets", [("time", time)]),
    (_, None) => String::new(),
  };
  plan_window(
    &format!("account-window-{}", window.minutes()),
    &PlanWindow {
      name: &name,
      used,
      reached,
      resets: &resets,
    },
  )
}

/// What a run on the account gets (`RXAmO`).
fn runs_get(ctx: &Ctx, account: &AgentAccount) -> Element {
  let cli = account.cli();
  let program = cli.program().to_owned();
  let variable = cli.folder_variable().to_owned();
  let own = orchester_workspace::accounts::own_folder(cli)
    .map(|folder| folder.display().to_string())
    .unwrap_or_default();
  let text = if account.is_default() {
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
  };
  Column::new()
    .id("account-runs-get")
    .width(Dimension::Pct(100.0))
    .spacing(crate::theme::ExtraSpacing::Snug.role())
    .child(Text::new(&t(ctx, "accounts.runsGet.title")).variant(SettingsText::SectionTitle.role()))
    .child(paragraph("account-runs-get-text", &text))
    .into()
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
