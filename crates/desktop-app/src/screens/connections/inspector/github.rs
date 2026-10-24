//! A GitHub connection's inspector (`cxjvN`, the states of `O1jDI`): its host, `gh` and its path, the account
//! it is signed in to and its scopes, when it was checked; the account and repository the project expects
//! through it; and in the footer what its state means, the `gh` command that fixes it to run in a terminal,
//! Check again and, for another account, *Use this account*. Orchester never signs in, switches accounts or
//! sees a token. Its header renames it; editing the connection otherwise and changing its account or access are not
//! built in this app yet.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{GitHubCheckResult, GitHubConnection, GitHubState, REQUIRED_SCOPE};
use orchester_workspace::display;

use super::{
  Actions, Inspecting, Parts, check_button, footer, footer_action, last_checked, rename::title_with_rename, result,
  shown, titled_header,
};
use crate::{
  screens::connections::{
    data::github_state,
    words::{GITHUB_GLYPH, github_status},
    work::Work,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::CodeBox,
    icon::Icon,
    inspector::{self, PropertyValue, property},
    unavailable::reason_line,
  },
};

const PATH_CHARS: usize = 26;

pub(super) fn parts(
  ctx: &mut Ctx,
  connection: &GitHubConnection,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
  let state = github_state(inspecting.listed, connection);
  let status = shown(inspecting, github_status(&state));
  let title = title_with_rename(ctx, inspecting, (connection.id(), connection.name().as_str()));
  let header = titled_header(ctx, (GITHUB_GLYPH, title), status, None);
  let mut body = vec![properties(ctx, connection, &state, inspecting).into()];
  body.extend(used_by_project(ctx, connection, inspecting));
  let command = state.advice().map(|advice| {
    CodeBox::new(
      "connection-gh-command",
      ctx.t("connections.runInTerminal"),
      advice.command(connection.host()),
    )
    .render(ctx)
  });
  let reason = ctx.t("connections.github.editUnavailable");
  let locked = |id: &str, key: &str, glyph: Icon| -> Element {
    footer_action(
      inspecting,
      ActionButton::new(id.to_owned(), ButtonKind::Secondary, ctx.t(key)),
      glyph,
    )
    .unavailable(reason.clone())
    .into()
  };
  // gh that is missing or answers as another program is pointed at another file; one that answers is edited.
  let other = match state {
    GitHubState::ProgramMissing | GitHubState::Unrecognized => locked(
      "connection-choose-program",
      "connections.github.chooseProgram",
      Icon::FolderOpen,
    ),
    _ => locked("connection-edit", "connections.edit", Icon::Pencil),
  };
  let check = footer_action(
    inspecting,
    check_button(
      inspecting,
      ("connection-check", ctx.t("connections.checkAgain")),
      actions,
    ),
    Icon::RefreshCw,
  );
  let mut rows = vec![vec![check.into(), other]];
  if matches!(
    state,
    GitHubState::SignedIn { .. } | GitHubState::WrongAccount { .. } | GitHubState::MissingScope { .. }
  ) {
    rows.push(vec![locked(
      "connection-change-account",
      "connections.github.changeAccount",
      Icon::UserRoundCog,
    )]);
  }
  if let GitHubState::WrongAccount { active, .. } = &state {
    let label = ctx.t_args("connections.github.useAccount", [("account", active.as_str())]);
    let button = footer_action(
      inspecting,
      ActionButton::new("connection-use-account", ButtonKind::Secondary, label),
      Icon::UserRoundCheck,
    );
    rows.push(vec![match inspecting.running {
      Some(Work::UseActiveAccount) => button.loading().into(),
      Some(_) => button.disabled().into(),
      None => {
        let (start, id) = (actions.start.clone(), connection.id());
        button.on_click(move || start(id, Work::UseActiveAccount)).into()
      }
    }]);
  }
  let more = command
    .into_iter()
    .chain(result::github(ctx, connection, &state, inspecting))
    .collect();
  let reasons = vec![reason_line("connection-edit-reason", &reason).into()];
  let sentence = sentence(ctx, &state, inspecting.project);
  Parts {
    header,
    body,
    footer: Some(footer(ctx, inspecting, (&sentence, more), (rows, reasons))),
  }
}

fn properties(ctx: &Ctx, connection: &GitHubConnection, state: &GitHubState, inspecting: &Inspecting<'_>) -> Column {
  let check = connection.last_check();
  let missing = matches!(state, GitHubState::ProgramMissing);
  let (program, program_kind) = match check.map(|check| &check.result) {
    Some(GitHubCheckResult::Available { version, .. }) => (format!("gh {version}"), PropertyValue::Mono),
    Some(GitHubCheckResult::ExecutableMissing) => {
      (ctx.t("connections.status.notFound").to_string(), PropertyValue::Danger)
    }
    _ => (ctx.t("connections.unknown").to_string(), PropertyValue::Muted),
  };
  let path_kind = if missing {
    PropertyValue::MonoDanger
  } else {
    PropertyValue::Mono
  };
  let (account, account_kind) = account(ctx, state);
  let (scopes, scopes_kind) = match check.map(|check| &check.result) {
    Some(GitHubCheckResult::Available { scopes, .. }) if !scopes.is_empty() => {
      let kind = if matches!(state, GitHubState::MissingScope { .. }) {
        PropertyValue::Warning
      } else {
        PropertyValue::Mono
      };
      (scopes.join(", "), kind)
    }
    _ => (ctx.t("connections.github.noScopes").to_string(), PropertyValue::Muted),
  };
  let (checked, checked_kind) = last_checked(ctx, inspecting.now, check.map(|check| check.at));
  Column::new()
    .id("connection-properties")
    .width(Dimension::Pct(100.0))
    .child(property(
      "connection-host",
      &ctx.t("connections.property.host"),
      connection.host().as_str(),
      PropertyValue::Plain,
    ))
    .child(property(
      "connection-program",
      &ctx.t("connections.property.program"),
      &program,
      program_kind,
    ))
    .child(property(
      "connection-path",
      &ctx.t("connections.property.path"),
      &display::shorten_path(connection.executable(), PATH_CHARS),
      path_kind,
    ))
    .child(property(
      "connection-account",
      &ctx.t("connections.property.account"),
      &account,
      account_kind,
    ))
    .child(property(
      "connection-scopes",
      &ctx.t("connections.property.scopes"),
      &scopes,
      scopes_kind,
    ))
    .child(property(
      "connection-checked",
      &ctx.t("connections.property.lastChecked"),
      &checked,
      checked_kind,
    ))
}

fn account(ctx: &Ctx, state: &GitHubState) -> (String, PropertyValue) {
  match state {
    GitHubState::SignedIn { account } | GitHubState::MissingScope { account } => {
      (account.to_string(), PropertyValue::Plain)
    }
    GitHubState::WrongAccount { active, .. } => (active.to_string(), PropertyValue::Warning),
    GitHubState::NotSignedIn => (
      ctx.t("connections.github.status.notSignedIn").to_string(),
      PropertyValue::Warning,
    ),
    GitHubState::ProgramMissing => (
      ctx.t("connections.github.accountUntilFound").to_string(),
      PropertyValue::Muted,
    ),
    GitHubState::NotChecked => (ctx.t("connections.notCheckedYet").to_string(), PropertyValue::Muted),
    GitHubState::SignInUnknown | GitHubState::Unrecognized | GitHubState::CouldNotRun => {
      (ctx.t("connections.unknown").to_string(), PropertyValue::Muted)
    }
  }
}

/// The account and repository the project expects through the connection, when its link goes through it.
fn used_by_project(ctx: &Ctx, connection: &GitHubConnection, inspecting: &Inspecting<'_>) -> Option<Element> {
  let link = inspecting.listed.link_of(connection.id())?;
  let repo = link.repo();
  let title = ctx.t_args("connections.github.usedByProject", [("project", inspecting.project)]);
  Some(
    inspector::section("connection-github-project", SpacingSize::Xs)
      .child(inspector::section_title(
        "connection-github-project-title",
        &title,
        None,
      ))
      .child(property(
        "connection-expected-account",
        &ctx.t("connections.github.expectedAccount"),
        link.expected_login().as_str(),
        PropertyValue::Plain,
      ))
      .child(property(
        "connection-repository",
        &ctx.t("connections.github.repository"),
        &ctx.t_args(
          "connections.github.repositoryName",
          [("owner", repo.owner()), ("name", repo.name())],
        ),
        PropertyValue::Plain,
      ))
      .child(property(
        "connection-remote",
        &ctx.t("connections.github.remote"),
        link.remote().as_str(),
        PropertyValue::Mono,
      ))
      .child(property(
        "connection-used-by-tool",
        &ctx.t("connections.github.usedByTool"),
        &ctx.t("connections.github.toolsUnavailable"),
        PropertyValue::Muted,
      ))
      .into(),
  )
}

/// What the state means and what to do about it.
fn sentence(ctx: &Ctx, state: &GitHubState, project: &str) -> std::sync::Arc<str> {
  match state {
    GitHubState::NotChecked => ctx.t("connections.detail.notChecked"),
    GitHubState::SignedIn { .. } => ctx.t("connections.github.detail.signedIn"),
    GitHubState::NotSignedIn => ctx.t("connections.github.detail.notSignedIn"),
    GitHubState::SignInUnknown => ctx.t("connections.github.detail.signInUnknown"),
    GitHubState::WrongAccount { active, expected } => ctx.t_args(
      "connections.github.detail.wrongAccount",
      [
        ("active", active.as_str()),
        ("expected", expected.as_str()),
        ("project", project),
      ],
    ),
    GitHubState::MissingScope { .. } => {
      ctx.t_args("connections.github.detail.missingScope", [("scope", REQUIRED_SCOPE)])
    }
    GitHubState::ProgramMissing => ctx.t("connections.github.detail.notFound"),
    GitHubState::Unrecognized => ctx.t("connections.github.detail.unrecognized"),
    GitHubState::CouldNotRun => ctx.t("connections.github.detail.couldNotRun"),
  }
}
