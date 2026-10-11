//! Stored GitHub link, actual origin checks and the expected-account action, all in place.
use std::path::PathBuf;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{connection::GitHubState, repository::DetectionState};
use orchester_workspace::{
  background,
  messages::{self, Message},
  project_settings::{self, RepositoryRead, Settings},
  tasks::outside::Outside,
};

use super::super::{
  TabProps, page,
  work::{self, Change, Save},
};
use crate::{
  services::Services,
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::{Place, section as form_section},
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
struct Read(RepositoryRead, u64);
impl DevtoolsInspectable for Read {}
type Check = FutureAction<(PathBuf, u64), Read, Message>;

pub fn section(ctx: &mut Ctx, props: &TabProps, settings: &Settings, save: &Save, revision: u64) -> Element {
  let changes = Services::of(ctx).changes;
  let project = props.project.open.project.id();
  let check: Check = ctx.future_action(move |(root, revision): (PathBuf, u64)| {
    let changes = changes.clone();
    async move {
      let read = background::run(move || project_settings::check_repository_in(&root).map(|read| Read(read, revision)))
        .await??;
      changes.record(project, Outside::Connections);
      Ok(read)
    }
  });
  let checked = check.state().get();
  let state = ctx
    .future(
      (
        props.project.open.repo_root.clone(),
        settings.connections.clone(),
        revision,
        props.project.feed.outside.get().of(Outside::Connections),
        checked.data.clone(),
      ),
      |(root, _, revision, ..)| async move {
        background::run(move || project_settings::repository_in(&root).map(|read| Read(read, revision)))
          .await?
          .map_err(|e| messages::store_error(&e))
      },
    )
    .state()
    .get();
  let fresh = state.data.as_ref().filter(|read| read.1 == revision);
  let checked_read = checked.data.as_ref().filter(|read| read.1 == revision);
  // A check's snapshot is useful during its readback, but cannot override later saves.
  let data = if state.is_pending() {
    checked_read.or(fresh)
  } else {
    fresh.or(checked_read)
  };
  let mut rows = match data {
    Some(read) => {
      let mut shown = read.0.clone();
      shown.detection = checked_read.and_then(|read| read.0.detection.clone());
      linked_rows(ctx, props, save, &check, &shown, revision)
    }
    None => vec![crate::ui::state::loading("settings-github-loading", &ctx.t("project_settings.reading")).into()],
  };
  if let Some(error) = checked.error.as_ref().or(state.error.as_ref()) {
    rows.push(field_error("settings-github-error", &message_text(ctx, error)).into());
  }
  form_section("settings-github", &ctx.t("project_settings.github"), None, rows).into()
}
fn linked_rows(
  ctx: &mut Ctx,
  props: &TabProps,
  save: &Save,
  check: &Check,
  read: &RepositoryRead,
  revision: u64,
) -> Vec<Element> {
  let repo_control = repository_control(ctx, props, check, read, revision);
  let account_control = account_control(ctx, props, save, read);
  let mut rows = vec![
    page::row(
      ctx,
      "settings-github-repo",
      ("project_settings.githubRepo", "project_settings.githubRepoDetail"),
      repo_control.into(),
      Place::Within,
    ),
    page::row(
      ctx,
      "settings-github-login",
      ("project_settings.githubAccount", "project_settings.githubAccountDetail"),
      account_control.into(),
      Place::Last,
    ),
  ];
  if let Some(detection) = &read.detection {
    rows.push(page::note(
      "settings-github-detection",
      &detection_text(ctx, &detection.state(read.link.as_ref().map(|l| l.expected_login()))),
    ));
  }
  rows
}
fn account_status(ctx: &Ctx, state: &GitHubState) -> Element {
  let text = match state {
    GitHubState::SignedIn { account } => ctx.t_args("project_settings.githubSignedIn", [("account", account.as_str())]),
    GitHubState::WrongAccount { active, expected } => ctx.t_args(
      "project_settings.githubWrongAccount",
      [("active", active.as_str()), ("expected", expected.as_str())],
    ),
    GitHubState::MissingScope { .. } => ctx.t("connections.github.detail.missingScope"),
    GitHubState::NotSignedIn => ctx.t("connections.github.detail.notSignedIn"),
    GitHubState::NotChecked => ctx.t("connections.detail.notChecked"),
    GitHubState::SignInUnknown => ctx.t("connections.github.detail.signInUnknown"),
    GitHubState::ProgramMissing => ctx.t("connections.github.detail.notFound"),
    GitHubState::Unrecognized => ctx.t("connections.github.detail.unrecognized"),
    GitHubState::CouldNotRun => ctx.t("connections.github.detail.couldNotRun"),
  };
  let (glyph, tone) = if matches!(state, GitHubState::SignedIn { .. }) {
    (Icon::CircleCheck, PaletteColor::TextSecondary)
  } else {
    (Icon::CircleAlert, PaletteColor::Warning)
  };
  Row::new()
    .id("settings-github-account-readback")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, tone.clone()))
    .child(Text::new(&text).variant(TypographyStyle::Caption).color(tone).flex(1.0))
    .into()
}
fn detection_text(ctx: &Ctx, state: &DetectionState) -> std::sync::Arc<str> {
  let key = match state {
    DetectionState::Detected { .. } => "project_settings.repositoryDetected",
    DetectionState::NoRemote | DetectionState::NotGitHub => "project_settings.noGithubRemote",
    DetectionState::NotAGitRepository => "project_settings.gitRepositoryMissing",
    DetectionState::GitFailed => "project_settings.gitCheckFailed",
    DetectionState::GhNotFound => "connections.github.detail.notFound",
    DetectionState::NotSignedIn => "connections.github.detail.notSignedIn",
    DetectionState::WrongAccount { .. } => "connections.github.status.wrongAccount",
    DetectionState::MissingScope { .. } => "connections.github.status.missingScope",
    DetectionState::NoAccess { .. } => "project_settings.repositoryNoAccess",
    DetectionState::Unrecognized | DetectionState::CouldNotRun => "project_settings.gitCheckFailed",
    DetectionState::Unreachable => "project_settings.githubUnreachable",
    DetectionState::RateLimited => "project_settings.githubRateLimited",
    DetectionState::NoDefaultBranch => "project_settings.noDefaultBranch",
  };
  ctx.t(key)
}
fn repository_control(ctx: &mut Ctx, props: &TabProps, check: &Check, read: &RepositoryRead, revision: u64) -> Column {
  let repository = read
    .link
    .as_ref()
    .map(|l| l.repo().to_string())
    .or_else(|| {
      read
        .detection
        .as_ref()
        .and_then(|d| d.github())
        .map(|d| d.repo().to_string())
    })
    .unwrap_or_else(|| ctx.t("project_settings.notLinked").to_string());
  let mut repo_control = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(super::locked(
      ctx,
      "settings-github-repository",
      "project_settings.githubRepo",
      &repository,
    ));
  if let Some(link) = &read.link {
    repo_control = repo_control.child(page::note(
      "settings-github-repository-readback",
      &ctx.t_args(
        "project_settings.repositoryReadback",
        [
          ("host", link.repo().host().as_str()),
          ("remote", link.remote().as_str()),
          ("branch", link.default_branch().as_str()),
        ],
      ),
    ));
  } else {
    repo_control = repo_control.child(page::note(
      "settings-github-repository-readback",
      &ctx.t("project_settings.repositoryNotLinked"),
    ));
  }
  let (action, root) = (check.clone(), props.project.open.repo_root.clone());
  let button = ActionButton::new(
    "settings-github-check",
    ButtonKind::Secondary,
    ctx.t("project_settings.checkAgain"),
  )
  .icon(Icon::RefreshCw)
  .in_form();
  repo_control = repo_control.child(if check.state().get().is_pending() {
    button.loading()
  } else {
    button.on_click(move || action.run((root.clone(), revision)))
  });
  repo_control
}
fn account_control(ctx: &mut Ctx, props: &TabProps, save: &Save, read: &RepositoryRead) -> Column {
  let account = read
    .link
    .as_ref()
    .map(|l| l.expected_login().as_str().to_owned())
    .unwrap_or_else(|| ctx.t("project_settings.none").to_string());
  let mut account_control = Column::new()
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm)
    .child(super::locked(
      ctx,
      "settings-github-account",
      "project_settings.githubAccount",
      &account,
    ));
  if let Some(connection) = &read.connection {
    let state = connection.state(read.link.as_ref().map(|l| l.expected_login()));
    account_control = account_control.child(account_status(ctx, &state));
    if matches!(state, GitHubState::WrongAccount { .. }) {
      let start = work::start(
        save,
        props.project.open.repo_root.clone(),
        Change::UseAccount(connection.id()),
      );
      let button = ActionButton::new(
        "settings-use-account",
        ButtonKind::Secondary,
        ctx.t("project_settings.useAccount"),
      )
      .in_form();
      account_control = account_control.child(if save.state().get().is_pending() {
        button.loading()
      } else {
        button.on_click(move || start())
      });
    }
  }
  account_control
}
