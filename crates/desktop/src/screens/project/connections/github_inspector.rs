//! Selected GitHub connection (inspector `C0jjP` in 04.05 `CHa9s`, states 04.C1–C5 `Cr0P2`,
//! `ve4G9`, `P89Dl`, `kqfaL`, `HwBI7`): host, program, account and scopes, what the project
//! expects when its repository is linked through it and what to do about the current state, in
//! a details viewport with a scroll gutter (`KAFgT`), and the actions pinned below it (`RlQQ2`).
//! Commands are shown to be copied; Orchester never runs them.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, ScrollVertical, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{GitHubCheckResult, GitHubConnection, GitHubState, REQUIRED_SCOPE},
  repository::RepositoryLink,
  time::Timestamp,
};
use orchester_exec::launch::Launch;

use super::{launch::launch_properties, status::CHECKING};
use crate::{
  display,
  messages::Message,
  screens::{
    github::status::{command_ids, status_label},
    project::page::TOPBAR_BUTTON_HEIGHT,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::ActionButton,
    command_row::{CommandPlace, Copied, command_row},
    field::field_error,
    icon::Icon,
    message_text,
    property::{ValueStyle, property, styled_property},
    status_label::status_label as label,
  },
};

/// Inspector width (`C0jjP`) and the longest path shown in full.
const INSPECTOR_WIDTH: f32 = 344.0;
const PATH_CHARS: usize = 34;

type Handler = Arc<dyn Fn() + Send + Sync>;

pub struct GitHubActions {
  /// `None` while a check or another action on the connection runs.
  pub check: Option<Handler>,
  pub change_account: Handler,
  pub choose_program: Option<Handler>,
  pub use_account: Option<Handler>,
  pub edit: Handler,
}

pub struct GitHubInspector<'a> {
  pub connection: &'a GitHubConnection,
  pub link: Option<&'a RepositoryLink>,
  pub project: &'a str,
  pub checking: bool,
  /// Why the last action on the connection failed.
  pub error: Option<Message>,
  pub copied: &'a Signal<Copied>,
  /// What starts when `gh` runs.
  pub launch: Option<&'a Launch>,
}

impl GitHubInspector<'_> {
  pub fn render(&self, ctx: &Ctx, actions: &GitHubActions) -> Element {
    let state = self.connection.state(self.link.map(RepositoryLink::expected_login));
    let (status_icon, status_key) = if self.checking { CHECKING } else { status_label(&state) };
    let mut details = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(Text::new(self.connection.name().as_str()).variant(ExtraTypography::InspectorTitle.role()))
      .child(label(status_icon, &ctx.t_args(status_key, [("scope", REQUIRED_SCOPE)])))
      .child(self.properties(ctx, &state));
    if let Some(link) = self.link {
      details = details
        .child(
          Text::new(&ctx.t_args("github.connection.usedBy", [("project", self.project)]))
            .variant(TypographyStyle::Label),
        )
        .child(link_properties(ctx, link));
    }
    details = details.child(
      Text::new(&self.hint(ctx, &state))
        .id("github-state-hint")
        .variant(ExtraTypography::Detail.role()),
    );
    if let Some(advice) = state.advice() {
      let (id, copy_id) = command_ids(&advice);
      let command = advice.command(self.connection.host());
      details = details.child(command_row(ctx, id, copy_id, &command, CommandPlace::Page, self.copied));
    }
    let pinned = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_right(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Md)
      .with_children(buttons(ctx, &state, actions))
      .with_children(self.error.as_ref().map(|error| field_error(&message_text(ctx, error))));
    // The details scroll when a state has more to say than fits, their thumb in a gutter of its
    // own (`yqhMN`) beside the right padding; the actions stay at the bottom.
    Column::new()
      .id("connection-inspector")
      .width(Dimension::Px(INSPECTOR_WIDTH))
      .height(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Lg)
      .padding_left(SpacingSize::Lg)
      .padding_right(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Md)
      .background(PaletteColor::SurfacePanel)
      .border_left(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        ScrollVertical::new(details)
          .id("github-inspector-details")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .scrollbar(gutter_scrollbar()),
      )
      .child(pinned)
      .into()
  }

  /// Host, program, path, what runs it, account, scopes and last check (`eOpdt`).
  fn properties(&self, ctx: &Ctx, state: &GitHubState) -> Element {
    let connection = self.connection;
    let result = connection.last_check().map(|check| &check.result);
    let program = match result {
      Some(GitHubCheckResult::Available { version, .. }) => {
        ctx.t_args("github.connection.program", [("version", version)])
      }
      Some(GitHubCheckResult::ExecutableMissing) => ctx.t("github.connection.programMissing"),
      Some(GitHubCheckResult::NotRecognized | GitHubCheckResult::CouldNotRun) | None => {
        ctx.t("github.connection.unknown")
      }
    };
    let checked = connection.last_check().map_or_else(
      || ctx.t("connections.notCheckedYet"),
      |check| message_text(ctx, &display::elapsed(Timestamp::now(), check.at)),
    );
    let mut rows = vec![
      property(&ctx.t("github.connection.host"), connection.host().as_str()),
      property(&ctx.t("connections.property.program"), &program),
      styled_property(
        &ctx.t("github.connection.path"),
        &display::shorten_path(connection.executable(), PATH_CHARS),
        ValueStyle::Path,
      ),
    ];
    rows.extend(launch_properties(ctx, self.launch, PATH_CHARS));
    if let Some(GitHubCheckResult::Available { account, scopes, .. }) = result {
      let account = match (state, account) {
        (GitHubState::NotSignedIn, _) | (_, None) => ctx.t("github.connection.notSignedIn"),
        (_, Some(account)) => account.as_str().into(),
      };
      rows.push(property(&ctx.t("github.connection.account"), &account));
      if !matches!(state, GitHubState::NotSignedIn) {
        let scopes = if scopes.is_empty() {
          ctx.t("github.connection.noScopes")
        } else {
          scopes.join(", ").into()
        };
        rows.push(property(&ctx.t("github.connection.scopes"), &scopes));
      }
    }
    rows.push(property(&ctx.t("connections.property.lastChecked"), &checked));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inline.role())
      .with_children(rows)
      .into()
  }

  /// What the state means and what to do (`uvFb6`, `Tw2UU`, `xiwXD`, `k3MwL`, `P84nc`, `e8S2F`).
  fn hint(&self, ctx: &Ctx, state: &GitHubState) -> Arc<str> {
    match state {
      GitHubState::NotChecked => ctx.t("github.hint.notChecked"),
      GitHubState::SignedIn { .. } => ctx.t("github.hint.signedIn"),
      GitHubState::NotSignedIn => ctx.t("github.hint.notSignedIn"),
      GitHubState::SignInUnknown => ctx.t("github.hint.signInUnknown"),
      GitHubState::WrongAccount { active, expected } => ctx.t_args(
        "github.hint.wrongAccount",
        [
          ("active", active.as_str()),
          ("expected", expected.as_str()),
          ("project", self.project),
        ],
      ),
      GitHubState::MissingScope { .. } => ctx.t_args("github.hint.missingScope", [("scope", REQUIRED_SCOPE)]),
      GitHubState::ProgramMissing => ctx.t("github.hint.programMissing"),
      GitHubState::Unrecognized => ctx.t("github.hint.unrecognized"),
      GitHubState::CouldNotRun => ctx.t("github.hint.couldNotRun"),
    }
  }
}

/// Expected account, repository and remote of the link through this connection (`WtiPq`).
fn link_properties(ctx: &Ctx, link: &RepositoryLink) -> Element {
  let repo = link.repo();
  let repository = ctx.t_args(
    "github.connection.repositoryName",
    [("owner", repo.owner()), ("name", repo.name())],
  );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .child(property(
      &ctx.t("github.connection.expectedAccount"),
      link.expected_login().as_str(),
    ))
    .child(property(&ctx.t("github.connection.repository"), &repository))
    .child(property(&ctx.t("github.connection.remote"), link.remote().as_str()))
    .into()
}

/// Check again, then what the state offers, then Edit connection (`nOZ00`, `wUapc`, `H4bIeb`,
/// `MGtNx`, `jPeVy`).
fn buttons(ctx: &Ctx, state: &GitHubState, actions: &GitHubActions) -> Vec<Element> {
  let gh_answers = !matches!(
    state,
    GitHubState::NotChecked | GitHubState::ProgramMissing | GitHubState::Unrecognized | GitHubState::CouldNotRun
  );
  let mut buttons = vec![button(
    ctx,
    "test-connection",
    ctx.t("github.checkAgain"),
    Icon::RefreshCw,
    actions.check.clone(),
  )];
  if matches!(state, GitHubState::ProgramMissing) {
    buttons.push(button(
      ctx,
      "github-choose-program",
      ctx.t("github.connection.chooseProgram"),
      Icon::FileSearch,
      actions.choose_program.clone(),
    ));
  }
  if gh_answers {
    buttons.push(button(
      ctx,
      "github-change-account",
      ctx.t("github.connection.changeAccount"),
      Icon::UserCog,
      Some(actions.change_account.clone()),
    ));
  }
  if let GitHubState::WrongAccount { active, .. } = state {
    buttons.push(button(
      ctx,
      "github-use-account",
      ctx.t_args("github.connection.useAccount", [("account", active.as_str())]),
      Icon::UserCheck,
      actions.use_account.clone(),
    ));
  }
  buttons.push(button(
    ctx,
    "edit-connection",
    ctx.t("connections.edit"),
    Icon::Settings,
    Some(actions.edit.clone()),
  ));
  buttons
}

/// A full-width secondary button, disabled without a handler (while work on the connection runs).
fn button(ctx: &Ctx, id: &'static str, text: Arc<str>, glyph: Icon, handler: Option<Handler>) -> Element {
  let button = ActionButton::new(text)
    .icon(glyph)
    .id(id)
    .fill_width()
    .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT));
  match handler {
    Some(handler) => button.on_click(move || handler()).render(ctx),
    // Disabled, not unavailable: it waits for the running work.
    None => button.on_click(|| {}).enabled(false).render(ctx),
  }
}
