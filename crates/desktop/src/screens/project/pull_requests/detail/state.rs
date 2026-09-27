//! What the pull request screen says instead of, or above, a diff: why the latest refresh did not
//! read the pull request (the stored diff stays below the notice, and the context line says when
//! it was read, as the inbox's sync status does in 08.E–08.H), that the diff is still being read,
//! that it is too large to show here, or that it has no changes.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{GitHubState, REQUIRED_SCOPE},
  diff::{MAX_DIFF_FILES, MAX_DIFF_LINES},
  pull_request::{PullRequest, SyncFailure},
  repository::RepositoryError,
  time::Timestamp,
};

use super::super::{notice::server_error, view::minutes_until};
use crate::{
  pull_requests::detail::LinkedDetail,
  screens::github::status::status_label,
  ui::{icon::Icon, notice::Notice, panel::RecoveryPanel},
};

/// Icon, heading and explanation of a failed refresh.
pub struct FailureText {
  pub icon: Icon,
  pub heading: Arc<str>,
  pub detail: Arc<str>,
}

/// The notice above a stored diff when the latest refresh did not read the pull request or its
/// diff.
pub fn failure_notice(ctx: &Ctx, detail: &LinkedDetail, now: Timestamp) -> Option<Element> {
  let failure = detail.latest_failure()?;
  let text = failure_text(ctx, detail, failure, now);
  Some(
    Notice::new(text.icon, text.heading, text.detail)
      .id("pull-request-notice")
      .into(),
  )
}

/// The panel in place of the diff when the latest refresh failed and nothing is stored.
pub fn failure_panel(ctx: &Ctx, detail: &LinkedDetail, failure: &SyncFailure, now: Timestamp) -> RecoveryPanel {
  let text = failure_text(ctx, detail, failure, now);
  RecoveryPanel::new(text.icon, &text.heading, &text.detail)
}

/// While the first refresh reads a pull request nothing is stored for.
pub fn loading_panel(ctx: &Ctx) -> RecoveryPanel {
  RecoveryPanel::new(
    Icon::Loader,
    &ctx.t("pullRequests.detail.loading.title"),
    &ctx.t("pullRequests.detail.loading.detail"),
  )
}

/// A pull request larger than Orchester reads (`Changes::is_too_large`), its sizes grouped as the locale
/// writes numbers ("20,000").
pub fn too_large_panel(ctx: &Ctx, pull_request: Option<&PullRequest>) -> RecoveryPanel {
  let separator = ctx.t("number.groupSeparator");
  let number = |value: u32| orchester_core::tokens::grouped(u64::from(value), &separator);
  let (max_files, max_lines) = (number(MAX_DIFF_FILES), number(MAX_DIFF_LINES));
  let limits = [("maxFiles", max_files.as_str()), ("maxLines", max_lines.as_str())];
  let detail = match pull_request.map(PullRequest::changes) {
    Some(changes) => {
      let sizes = [
        number(changes.files),
        number(changes.additions),
        number(changes.deletions),
      ];
      ctx.t_args(
        "pullRequests.detail.tooLarge.detail",
        [
          ("files", sizes[0].as_str()),
          ("additions", sizes[1].as_str()),
          ("deletions", sizes[2].as_str()),
          limits[0],
          limits[1],
        ],
      )
    }
    None => ctx.t_args("pullRequests.detail.tooLarge.limits", limits),
  };
  RecoveryPanel::new(Icon::FileText, &ctx.t("pullRequests.detail.tooLarge.title"), &detail)
}

/// A diff without files: the head has no changes against its base.
pub fn empty_panel(ctx: &Ctx) -> RecoveryPanel {
  RecoveryPanel::new(
    Icon::CircleCheck,
    &ctx.t("pullRequests.detail.empty.title"),
    &ctx.t("pullRequests.detail.empty.detail"),
  )
}

fn failure_text(ctx: &Ctx, detail: &LinkedDetail, failure: &SyncFailure, now: Timestamp) -> FailureText {
  let account = detail.expected.as_str();
  let text = |icon, heading, detail| FailureText { icon, heading, detail };
  match failure {
    SyncFailure::RateLimited { resets_at: Some(at) } => text(
      Icon::Timer,
      ctx.t("pullRequests.notice.rateLimitedTitle"),
      ctx.t_args(
        "pullRequests.detail.failure.rateLimited",
        [
          ("account", account),
          ("minutes", minutes_until(now, *at).to_string().as_str()),
        ],
      ),
    ),
    SyncFailure::RateLimited { resets_at: None } => text(
      Icon::Timer,
      ctx.t("pullRequests.notice.rateLimitedTitle"),
      ctx.t_args("pullRequests.notice.rateLimitedUnknown", [("account", account)]),
    ),
    SyncFailure::Account(state) => {
      let (glyph, title) = status_label(state);
      let heading = match state {
        GitHubState::WrongAccount { active, expected } => ctx.t_args(
          "pullRequests.notice.wrongAccountTitle",
          [("active", active.as_str()), ("expected", expected.as_str())],
        ),
        _ => ctx.t_args(title, [("scope", REQUIRED_SCOPE)]),
      };
      text(
        glyph,
        heading,
        ctx.t_args("pullRequests.detail.failure.account", [("account", account)]),
      )
    }
    SyncFailure::Repository(error) => repository(ctx, detail, error),
  }
}

fn repository(ctx: &Ctx, detail: &LinkedDetail, error: &RepositoryError) -> FailureText {
  let number = detail.number.to_string();
  let (owner, name) = (detail.repo.owner(), detail.repo.name());
  let not_read = || ctx.t_args("pullRequests.detail.failure.notRead", [("number", number.as_str())]);
  let (icon, heading, explanation) = match error {
    RepositoryError::Network { .. } => (
      Icon::CloudOff,
      ctx.t("pullRequests.detail.failure.unreachableTitle"),
      ctx.t("pullRequests.detail.failure.unreachable"),
    ),
    RepositoryError::ServerError { status, .. } => (
      Icon::CloudOff,
      ctx.t("pullRequests.detail.failure.troubledTitle"),
      server_error(
        ctx,
        *status,
        [
          "pullRequests.detail.failure.troubled",
          "pullRequests.detail.failure.troubledUnknown",
        ],
      ),
    ),
    RepositoryError::PullRequestNotFound { .. } => (
      Icon::FileSearch,
      ctx.t_args(
        "pullRequests.detail.failure.notFoundTitle",
        [("number", number.as_str())],
      ),
      ctx.t_args(
        "pullRequests.detail.failure.notFound",
        [
          ("number", number.as_str()),
          ("owner", owner),
          ("name", name),
          ("account", detail.expected.as_str()),
        ],
      ),
    ),
    RepositoryError::DiffNotFound(_) => (
      Icon::FileSearch,
      ctx.t("pullRequests.detail.failure.diffNotFoundTitle"),
      ctx.t("pullRequests.detail.failure.diffNotFound"),
    ),
    RepositoryError::NotFound(_) | RepositoryError::NoAccess(_) => (
      Icon::FolderLock,
      ctx.t_args(
        "pullRequests.notice.noAccessTitle",
        [("account", detail.expected.as_str()), ("owner", owner), ("name", name)],
      ),
      ctx.t("pullRequests.notice.noAccess"),
    ),
    RepositoryError::NotSignedIn(host) => (
      Icon::AlertCircle,
      ctx.t_args("pullRequests.notice.notSignedInTitle", [("host", host.as_str())]),
      ctx.t("github.hint.notSignedIn"),
    ),
    RepositoryError::Unrecognized { .. } => (Icon::CircleHelp, not_read(), ctx.t("pullRequests.notice.unrecognized")),
    RepositoryError::Run { .. } => (Icon::AlertCircle, not_read(), ctx.t("pullRequests.notice.couldNotRun")),
    _ => (
      Icon::AlertCircle,
      not_read(),
      ctx.t("pullRequests.detail.failure.failed"),
    ),
  };
  FailureText {
    icon,
    heading,
    detail: explanation,
  }
}
