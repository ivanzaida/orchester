//! The repository's GitHub rows (`GitHub repository` `AuIZJ`, `GitHub account` `KjfsB` in `01 · General`; `FDH62` in
//! `01b`): the stored link with Check again, which reads the folder's origin again, and the account the project expects
//! with how `gh` matches it; when `gh` is signed in as another account, *Use* it takes the place of the value.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_core::{connection::GitHubState, repository::DetectionState};
use orchester_workspace::{
  background,
  messages::{self, Message},
  project_settings::{self, RepositoryRead, Settings},
  tasks::outside::Outside,
};

use super::super::{
  TabProps,
  kit::{SettingRow, Tone, value},
  work::{self, Change, Save},
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
    message::message_text,
    state::loading,
  },
};

#[derive(Clone, PartialEq)]
struct Read(RepositoryRead, u64);
impl DevtoolsInspectable for Read {}
type Check = FutureAction<(PathBuf, u64), Read, Message>;

/// The two rows, or one that says they are read; a failed read or check says why on the repository's.
pub fn rows(ctx: &mut Ctx, props: &TabProps, settings: &Settings, save: &Save, revision: u64) -> Vec<Element> {
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
  let error = checked
    .error
    .as_ref()
    .or(state.error.as_ref())
    .map(|error| message_text(ctx, error));
  let Some(read) = data else {
    let reading = SettingRow::new("settings-github-repo", ctx.t("project_settings.githubRepo"));
    let reading = match error {
      Some(error) => reading.toned(error, Tone::Danger),
      None => reading.under(loading("settings-github-loading", &ctx.t("project_settings.reading"))),
    };
    return vec![reading.render(ctx)];
  };
  let mut shown = read.0.clone();
  shown.detection = checked_read.and_then(|read| read.0.detection.clone());
  vec![
    repository_row(ctx, (props, &check, revision), &shown, error),
    account_row(ctx, props, save, &shown),
  ]
}

/// The repository with Check again; its hint says what a check found, why reading failed, or where pushes go.
fn repository_row(
  ctx: &mut Ctx,
  (props, check, revision): (&TabProps, &Check, u64),
  read: &RepositoryRead,
  error: Option<Arc<str>>,
) -> Element {
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
  let detection = read
    .detection
    .as_ref()
    .map(|detection| detection_text(ctx, &detection.state(read.link.as_ref().map(|l| l.expected_login()))));
  let (hint, tone) = match (error, detection, &read.link) {
    (Some(error), ..) => (error, Tone::Danger),
    (None, Some((words, tone)), _) => (words, tone),
    (None, None, Some(_)) => (ctx.t("project_settings.githubRepoHint"), Tone::Muted),
    (None, None, None) => (ctx.t("project_settings.repositoryNotLinked"), Tone::Warning),
  };
  let info = match &read.link {
    Some(link) => ctx.t_args(
      "project_settings.githubRepoInfo",
      [
        ("detail", ctx.t("project_settings.githubRepoDetail").to_string()),
        (
          "readback",
          ctx
            .t_args(
              "project_settings.repositoryReadback",
              [
                ("host", link.repo().host().as_str()),
                ("remote", link.remote().as_str()),
                ("branch", link.default_branch().as_str()),
              ],
            )
            .to_string(),
        ),
      ],
    ),
    None => ctx.t("project_settings.githubRepoDetail"),
  };
  let (action, root) = (check.clone(), props.project.open.repo_root.clone());
  let again = IconButton::new(
    "settings-github-check",
    Icon::RefreshCw,
    ctx.t("project_settings.checkAgain"),
  )
  .small();
  let again = if check.state().get().is_pending() {
    again
  } else {
    again.on_click(move || action.run((root.clone(), revision)))
  };
  let again = again.render(ctx);
  let shown = Row::new()
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::GitPullRequest, IconSize::Base, PaletteColor::TextSecondary))
    .child(value("settings-github-repository", &repository))
    .child(again);
  SettingRow::new("settings-github-repo", ctx.t("project_settings.githubRepo"))
    .info(info)
    .toned(hint, tone)
    .control(shown)
    .render(ctx)
}

/// The account the project expects, and how `gh` matches it; *Use* the signed-in account when it is another.
fn account_row(ctx: &mut Ctx, props: &TabProps, save: &Save, read: &RepositoryRead) -> Element {
  let account = read
    .link
    .as_ref()
    .map(|l| l.expected_login().as_str().to_owned())
    .unwrap_or_else(|| ctx.t("project_settings.none").to_string());
  let row = SettingRow::new("settings-github-login", ctx.t("project_settings.githubAccountLabel"))
    .info(ctx.t("project_settings.githubAccountDetail"));
  let Some(connection) = &read.connection else {
    return row.control(value("settings-github-account", &account)).render(ctx);
  };
  let state = connection.state(read.link.as_ref().map(|l| l.expected_login()));
  let (hint, tone) = account_status(ctx, &state);
  let row = row.toned(hint, tone);
  let GitHubState::WrongAccount { active, .. } = &state else {
    return row.control(value("settings-github-account", &account)).render(ctx);
  };
  let start = work::start(
    save,
    props.project.open.repo_root.clone(),
    Change::UseAccount(connection.id()),
  );
  let button = ActionButton::new(
    "settings-use-account",
    ButtonKind::Secondary,
    ctx.t_args("project_settings.useNamedAccount", [("account", active.as_str())]),
  )
  .icon(Icon::UserRoundCheck)
  .in_form();
  let button = if save.state().get().is_pending() {
    button.loading()
  } else {
    button.on_click(move || start())
  };
  row.control(button).render(ctx)
}

fn account_status(ctx: &Ctx, state: &GitHubState) -> (Arc<str>, Tone) {
  let text = match state {
    GitHubState::SignedIn { account } => ctx.t_args("project_settings.githubMatches", [("account", account.as_str())]),
    GitHubState::WrongAccount { active, expected } => ctx.t_args(
      "project_settings.githubWrongAccount",
      [("active", active.as_str()), ("expected", expected.as_str())],
    ),
    GitHubState::MissingScope { .. } => ctx.t_args(
      "connections.github.detail.missingScope",
      [("scope", orchester_core::connection::REQUIRED_SCOPE)],
    ),
    GitHubState::NotSignedIn => ctx.t("connections.github.detail.notSignedIn"),
    GitHubState::NotChecked => ctx.t("connections.detail.notChecked"),
    GitHubState::SignInUnknown => ctx.t("connections.github.detail.signInUnknown"),
    GitHubState::ProgramMissing => ctx.t("connections.github.detail.notFound"),
    GitHubState::Unrecognized => ctx.t("connections.github.detail.unrecognized"),
    GitHubState::CouldNotRun => ctx.t("connections.github.detail.couldNotRun"),
  };
  let tone = match state {
    GitHubState::SignedIn { .. } => Tone::Success,
    GitHubState::NotChecked => Tone::Muted,
    _ => Tone::Warning,
  };
  (text, tone)
}

/// What a check of the folder's origin found, in its tone.
fn detection_text(ctx: &Ctx, state: &DetectionState) -> (Arc<str>, Tone) {
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
  let tone = match state {
    DetectionState::Detected { .. } => Tone::Muted,
    DetectionState::NotAGitRepository | DetectionState::NoAccess { .. } => Tone::Danger,
    _ => Tone::Warning,
  };
  (ctx.t(key), tone)
}
