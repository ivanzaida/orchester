//! A GitHub connection's page (`14` `KItiZ`): Check again and its … menu (Rename; editing it, choosing another `gh` and
//! changing its account or access are not built yet, and say so); what its state asks for — signed in as another
//! account, a warning with *Use* that account (`Wrong account` `ZH4Hq`), or the state's sentence — with the `gh`
//! command that fixes it to run in a terminal; *Connection*: its host, `gh` and its path, the account it is signed in
//! to and its scopes, and the account and repository the project expects through it. Orchester never signs in,
//! switches accounts or sees a token.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::{GitHubCheckResult, GitHubConnection, GitHubState, REQUIRED_SCOPE};
use orchester_workspace::display;

use super::{
  Actions, Inspecting, Parts, check_button, last_checked, rename::title_with_rename, rename_item, result, sentence,
  sentence_tone, shown,
};
use crate::{
  screens::{
    connections::{
      data::github_state,
      words::{GITHUB_GLYPH, github_status},
      work::Work,
    },
    project_settings::kit::{Group, SettingRow, Tone, command_line, value},
  },
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    menu::MenuItem,
  },
};

const PATH_CHARS: usize = 40;

pub(super) fn parts(
  ctx: &mut Ctx,
  connection: &GitHubConnection,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
  let state = github_state(inspecting.listed, connection);
  let status = shown(inspecting, github_status(&state));
  let mut notices = match &state {
    GitHubState::WrongAccount { active, .. } => vec![wrong_account(ctx, connection, (active.as_str(), &state), (inspecting, actions))],
    GitHubState::SignedIn { .. } | GitHubState::NotChecked => Vec::new(),
    _ => vec![sentence(
      "connection-inspector-sentence",
      &said(ctx, &state, inspecting.project),
      sentence_tone(status),
    )],
  };
  if let Some(advice) = state.advice() {
    notices.push(command_line(
      ctx,
      "connection-gh-command",
      ctx.t("connections.runInTerminal"),
      advice.command(connection.host()),
    ));
  }
  notices.extend(result::github(ctx, connection, &state, inspecting));
  let detail = connection
    .last_check()
    .map(|check| ctx.t_args("connections.checkedAt", [("time", &*last_checked(ctx, inspecting.now, Some(check.at)))]));
  Parts {
    glyph: GITHUB_GLYPH,
    title: title_with_rename(ctx, (inspecting, actions), (connection.id(), connection.name().as_str())),
    status,
    status_detail: detail,
    actions: vec![check_button(inspecting, ("connection-check", ctx.t("connections.checkAgain")), actions).into()],
    menu: menu(ctx, &state, (inspecting, actions)),
    notices,
    groups: vec![connection_group(ctx, connection, &state, inspecting)],
    question: None,
  }
}

/// Rename, and what is not built yet for GitHub: editing it (or, for a `gh` that is missing or answers as another
/// program, choosing another one) and changing its account or access.
fn menu(ctx: &Ctx, state: &GitHubState, (inspecting, actions): (&Inspecting<'_>, &Actions)) -> Vec<MenuItem> {
  let reason = ctx.t("connections.github.editUnavailable");
  let other = match state {
    GitHubState::ProgramMissing | GitHubState::Unrecognized => {
      MenuItem::unavailable("connection-choose-program", ctx.t("connections.github.chooseProgram"), reason.clone())
        .icon(Icon::FolderOpen)
    }
    _ => MenuItem::unavailable("connection-edit", ctx.t("connections.edit"), reason.clone()).icon(Icon::Pencil),
  };
  let mut items = vec![rename_item(ctx, inspecting, actions), other];
  if matches!(
    state,
    GitHubState::SignedIn { .. } | GitHubState::WrongAccount { .. } | GitHubState::MissingScope { .. }
  ) {
    items.push(
      MenuItem::unavailable("connection-change-account", ctx.t("connections.github.changeAccount"), reason)
        .icon(Icon::UserRoundCog),
    );
  }
  items
}

/// `gh` signed in as another account than the project expects: pull requests wait, and *Use* the signed-in account.
fn wrong_account(
  ctx: &Ctx,
  connection: &GitHubConnection,
  (active, state): (&str, &GitHubState),
  (inspecting, actions): (&Inspecting<'_>, &Actions),
) -> Element {
  let label = ctx.t_args("connections.github.useAccount", [("account", active)]);
  let button = ActionButton::new("connection-use-account", ButtonKind::Secondary, label).icon(Icon::UserRoundCheck);
  let button = match inspecting.running {
    Some(Work::UseActiveAccount) => button.loading(),
    Some(_) => button.disabled(),
    None => {
      let (start, id) = (actions.start.clone(), connection.id());
      button.on_click(move || start(id, Work::UseActiveAccount))
    }
  };
  Row::new()
    .id("connection-wrong-account")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::WarningMuted)
    .rounded(lurq::app::theme::RadiusSize::Md)
    .child(icon(Icon::TriangleAlert, IconSize::Md, PaletteColor::Warning))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t("connections.github.wrongAccountTitle")).variant(ExtraTypography::OptionTitle.role()))
        .child(
          Text::new(&said(ctx, state, inspecting.project))
            .id("connection-inspector-sentence")
            .variant(ExtraTypography::Prose.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(button)
    .into()
}

/// *Connection* (`MqOPV`): the host, `gh` and its path, the account and scopes it is signed in with, and the account
/// and repository the project expects through it.
fn connection_group(ctx: &mut Ctx, connection: &GitHubConnection, state: &GitHubState, inspecting: &Inspecting<'_>) -> Element {
  let check = connection.last_check();
  let path = display::shorten_path(connection.executable(), PATH_CHARS);
  let (program, program_tone) = match check.map(|check| &check.result) {
    Some(GitHubCheckResult::Available { version, .. }) => (
      ctx.t_args("connections.github.programFound", [("version", version.as_str())]),
      Tone::Muted,
    ),
    Some(GitHubCheckResult::ExecutableMissing) => (ctx.t("connections.status.notFound"), Tone::Danger),
    _ => (ctx.t("connections.unknown"), Tone::Muted),
  };
  let (account, account_tone) = account(ctx, state);
  let (scopes, scopes_tone) = match check.map(|check| &check.result) {
    Some(GitHubCheckResult::Available { scopes, .. }) if !scopes.is_empty() => {
      let tone = if matches!(state, GitHubState::MissingScope { .. }) {
        Tone::Warning
      } else {
        Tone::Secondary
      };
      (scopes.join(", ").into(), tone)
    }
    _ => (ctx.t("connections.github.noScopes"), Tone::Muted),
  };
  let mut rows = vec![
    row(ctx, "connection-host", ("connections.property.host", connection.host().as_str()), Tone::Secondary, None),
    row(ctx, "connection-program", ("connections.property.program", &program), program_tone, Some(&path)),
    row(ctx, "connection-account", ("connections.property.account", &account), account_tone, None),
    row(ctx, "connection-scopes", ("connections.property.scopes", &scopes), scopes_tone, None),
  ];
  if let Some(link) = inspecting.listed.link_of(connection.id()) {
    let repo = link.repo();
    let repository = ctx.t_args(
      "connections.github.repositoryFrom",
      [
        (
          "repository",
          ctx
            .t_args(
              "connections.github.repositoryName",
              [("owner", repo.owner()), ("name", repo.name())],
            )
            .to_string(),
        ),
        ("remote", link.remote().as_str().to_owned()),
      ],
    );
    rows.push(row(
      ctx,
      "connection-expected-account",
      ("connections.github.expectedAccount", link.expected_login().as_str()),
      Tone::Secondary,
      None,
    ));
    rows.push(row(
      ctx,
      "connection-repository",
      ("connections.github.repository", &repository),
      Tone::Secondary,
      Some(&ctx.t("connections.github.toolsUnavailable")),
    ));
  }
  Group::new("connection-properties", ctx.t("connections.connectionGroup"))
    .rows(rows)
    .into()
}

/// A row with its value at the end in `tone`, and what it leaves out in its hint.
fn row(ctx: &mut Ctx, id: &str, (label, shown): (&str, &str), tone: Tone, hint: Option<&str>) -> Element {
  let row = SettingRow::new(id.to_owned(), ctx.t(label)).control(value(&format!("{id}-value"), shown).color(tone.color()));
  match hint {
    Some(hint) => row.hint(hint.to_owned()),
    None => row,
  }
  .render(ctx)
}

fn account(ctx: &Ctx, state: &GitHubState) -> (std::sync::Arc<str>, Tone) {
  match state {
    GitHubState::SignedIn { account } | GitHubState::MissingScope { account } => (account.as_str().into(), Tone::Secondary),
    GitHubState::WrongAccount { active, .. } => (active.as_str().into(), Tone::Warning),
    GitHubState::NotSignedIn => (ctx.t("connections.github.status.notSignedIn"), Tone::Warning),
    GitHubState::ProgramMissing => (ctx.t("connections.github.accountUntilFound"), Tone::Muted),
    GitHubState::NotChecked => (ctx.t("connections.notCheckedYet"), Tone::Muted),
    GitHubState::SignInUnknown | GitHubState::Unrecognized | GitHubState::CouldNotRun => {
      (ctx.t("connections.unknown"), Tone::Muted)
    }
  }
}

/// What the state means and what to do about it.
fn said(ctx: &Ctx, state: &GitHubState, project: &str) -> std::sync::Arc<str> {
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
