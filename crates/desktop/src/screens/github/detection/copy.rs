//! Words of the detected repository block: the status label, heading, note and properties of
//! each detection state (`XcbNT`, 01.C1–C10 `fL24i` … `MTPqL`). States beyond the design
//! (git failed, gh not answering, unreachable, rate limited, empty repository) use the same
//! parts.

use std::{path::Path, sync::Arc};

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::{
  connection::{GitHubCheckResult, GitHubLogin, REQUIRED_SCOPE},
  repository::{Detection, DetectionState, GitHubDetection, GitHubRepo},
};

use super::DetectionPlace;
use crate::{
  display,
  ui::{
    icon::Icon,
    property::{ValueStyle, styled_property},
  },
};

/// Longest folder path shown in full (`w1e9v`).
const PATH_CHARS: usize = 56;

/// Status label (`DeizF`) of the state; `None` while detecting.
pub fn status(state: Option<&DetectionState>) -> (Icon, &'static str) {
  let Some(state) = state else {
    return (Icon::Loader, "github.detection.status.checking");
  };
  match state {
    DetectionState::Detected { .. } => (Icon::CircleCheck, "github.detection.status.detected"),
    DetectionState::NoRemote => (Icon::CircleMinus, "github.detection.status.localOnly"),
    DetectionState::NotGitHub => (Icon::CircleMinus, "github.detection.status.notGitHub"),
    DetectionState::GhNotFound => (Icon::AlertCircle, "github.detection.status.ghNotFound"),
    DetectionState::NotSignedIn => (Icon::AlertCircle, "github.detection.status.notSignedIn"),
    DetectionState::WrongAccount { .. } => (Icon::AlertCircle, "github.detection.status.wrongAccount"),
    DetectionState::NoAccess { .. } => (Icon::Lock, "github.detection.status.noAccess"),
    DetectionState::Unrecognized => (Icon::CircleHelp, "github.detection.status.unrecognized"),
    DetectionState::NotAGitRepository => (Icon::FolderX, "github.detection.status.notRepository"),
    DetectionState::MissingScope { .. } => (Icon::KeyRound, "github.detection.status.missingScope"),
    DetectionState::GitFailed => (Icon::AlertCircle, "github.detection.status.gitFailed"),
    DetectionState::CouldNotRun => (Icon::AlertCircle, "github.detection.status.couldNotRun"),
    DetectionState::Unreachable => (Icon::CloudOff, "github.detection.status.unreachable"),
    DetectionState::RateLimited => (Icon::Clock3, "github.detection.status.rateLimited"),
    DetectionState::NoDefaultBranch => (Icon::CircleMinus, "github.detection.status.emptyRepository"),
  }
}

/// The block's heading (`o5dCc`).
pub fn heading(ctx: &Ctx, state: Option<&DetectionState>, found: Option<&GitHubDetection>) -> Arc<str> {
  let host = found.map_or_else(String::new, |found| found.repo().host().to_string());
  let Some(state) = state else {
    return ctx.t("github.detection.title");
  };
  match state {
    DetectionState::Detected { .. } => ctx.t("github.detection.title"),
    DetectionState::NoRemote => ctx.t("github.detection.heading.noRemote"),
    DetectionState::NotGitHub => ctx.t("github.detection.heading.notGitHub"),
    DetectionState::GhNotFound => ctx.t("github.detection.heading.ghNotFound"),
    DetectionState::NotSignedIn => ctx.t_args("github.detection.heading.notSignedIn", [("host", host.as_str())]),
    DetectionState::WrongAccount { active, expected } => ctx.t_args(
      "github.detection.heading.wrongAccount",
      [("active", active.as_str()), ("expected", expected.as_str())],
    ),
    DetectionState::NoAccess { account } => {
      ctx.t_args("github.detection.heading.noAccess", [("account", account.as_str())])
    }
    DetectionState::Unrecognized => ctx.t("github.detection.heading.unrecognized"),
    DetectionState::NotAGitRepository => ctx.t("github.detection.heading.notRepository"),
    DetectionState::MissingScope { .. } => {
      ctx.t_args("github.detection.heading.missingScope", [("scope", REQUIRED_SCOPE)])
    }
    DetectionState::GitFailed => ctx.t("github.detection.heading.gitFailed"),
    DetectionState::CouldNotRun => ctx.t("github.detection.heading.couldNotRun"),
    DetectionState::Unreachable => ctx.t_args("github.detection.heading.unreachable", [("host", host.as_str())]),
    DetectionState::RateLimited => ctx.t("github.detection.heading.rateLimited"),
    DetectionState::NoDefaultBranch => ctx.t("github.detection.heading.emptyRepository"),
  }
}

/// What the state means and what to do (`s4l5k`, `BP2mR`, `Ry9ZH`, `f5MbMu`, `QifIr`, `YO3pe`,
/// `WIbgw`, `Apk2H`, `xZ3Ol`, `KvuCf`, `Txwiy`). In Project settings the project exists, so the
/// notes that say it can be created are replaced.
pub fn note(
  ctx: &Ctx,
  state: Option<&DetectionState>,
  found: Option<&GitHubDetection>,
  project: &str,
  place: DetectionPlace,
) -> Arc<str> {
  let settings = place == DetectionPlace::Settings;
  let host = found.map_or_else(String::new, |found| found.repo().host().to_string());
  let Some(state) = state else {
    return if settings {
      ctx.t("github.detection.settingsNote.detecting")
    } else {
      ctx.t("github.detection.note.detecting")
    };
  };
  match state {
    DetectionState::Detected { account } => ctx.t_args(
      "github.detection.note.detected",
      [("project", project), ("account", account.as_str())],
    ),
    DetectionState::NoRemote if settings => ctx.t("github.detection.settingsNote.noRemote"),
    DetectionState::NoRemote => ctx.t("github.detection.note.noRemote"),
    DetectionState::NotGitHub => ctx.t("github.detection.note.notGitHub"),
    DetectionState::GhNotFound if settings => ctx.t("github.detection.settingsNote.ghNotFound"),
    DetectionState::GhNotFound => ctx.t("github.detection.note.ghNotFound"),
    DetectionState::NotSignedIn => ctx.t("github.detection.note.notSignedIn"),
    DetectionState::WrongAccount { active, expected } => ctx.t_args(
      "github.detection.note.wrongAccount",
      [
        ("project", project),
        ("active", active.as_str()),
        ("expected", expected.as_str()),
      ],
    ),
    DetectionState::NoAccess { .. } => ctx.t("github.detection.note.noAccess"),
    DetectionState::Unrecognized => match found.and_then(GitHubDetection::version) {
      Some(version) => ctx.t_args("github.detection.note.unrecognizedVersion", [("version", version)]),
      None => ctx.t("github.detection.note.unrecognized"),
    },
    DetectionState::NotAGitRepository if settings => ctx.t("github.detection.settingsNote.notRepository"),
    DetectionState::NotAGitRepository => ctx.t("github.detection.note.notRepository"),
    DetectionState::MissingScope { account } if settings => ctx.t_args(
      "github.detection.settingsNote.missingScope",
      [("account", account.as_str()), ("scope", REQUIRED_SCOPE)],
    ),
    DetectionState::MissingScope { account } => ctx.t_args(
      "github.detection.note.missingScope",
      [("account", account.as_str()), ("scope", REQUIRED_SCOPE)],
    ),
    DetectionState::GitFailed => ctx.t("github.detection.note.gitFailed"),
    DetectionState::CouldNotRun => ctx.t("github.detection.note.couldNotRun"),
    DetectionState::Unreachable => ctx.t_args("github.detection.note.unreachable", [("host", host.as_str())]),
    DetectionState::RateLimited => ctx.t("github.detection.note.rateLimited"),
    DetectionState::NoDefaultBranch => ctx.t("github.detection.note.emptyRepository"),
  }
}

/// The properties of the state (`Wz2Gv`, `HPRvY`, `b2t7cM`, …): what is known of the
/// repository, its remote and the account.
pub fn properties(
  ctx: &Ctx,
  detection: Option<&Detection>,
  state: Option<&DetectionState>,
  expected: Option<&GitHubLogin>,
  folder: &Path,
) -> Vec<Element> {
  let (Some(detection), Some(state)) = (detection, state) else {
    let checking = ctx.t("github.detection.value.checking");
    return [
      ctx.t("github.detection.property.repository"),
      ctx.t("github.detection.property.remote"),
      ctx.t("github.detection.property.defaultBranch"),
      ctx.t("github.detection.property.account"),
    ]
    .iter()
    .map(|label| styled_property(label, &checking, ValueStyle::Pending))
    .collect();
  };
  match detection {
    Detection::NotAGitRepository | Detection::GitFailed(_) => folder_properties(ctx, state, folder),
    Detection::NoRemote(remote) => vec![styled_property(
      &ctx.t("github.detection.property.remote"),
      &ctx.t_args("github.detection.value.noRemote", [("remote", remote.as_str())]),
      ValueStyle::Text,
    )],
    Detection::NotGitHub(remote) => vec![remote_property(
      ctx,
      remote.name().as_str(),
      display::remote_address(remote.url()),
    )],
    Detection::GitHub(found) => github_properties(ctx, found, state, expected),
  }
}

fn folder_properties(ctx: &Ctx, state: &DetectionState, folder: &Path) -> Vec<Element> {
  let mut rows = vec![styled_property(
    &ctx.t("github.detection.property.folder"),
    &display::shorten_path(folder, PATH_CHARS),
    ValueStyle::Mono,
  )];
  if matches!(state, DetectionState::NotAGitRepository) {
    rows.push(styled_property(
      &ctx.t("github.detection.property.git"),
      &ctx.t("github.detection.value.noGitRepository"),
      ValueStyle::Text,
    ));
  }
  rows
}

fn github_properties(
  ctx: &Ctx,
  found: &GitHubDetection,
  state: &DetectionState,
  expected: Option<&GitHubLogin>,
) -> Vec<Element> {
  let view = found.view();
  let repository = match (state, view) {
    (_, Some(view)) => repository_name(ctx, &view.repo),
    (DetectionState::NoAccess { .. }, None) => repository_name(ctx, found.repo()),
    (_, None) => ctx.t_args(
      "github.detection.value.fromRemote",
      [("owner", found.repo().owner()), ("name", found.repo().name())],
    ),
  };
  let mut rows = vec![
    styled_property(
      &ctx.t("github.detection.property.repository"),
      &repository,
      ValueStyle::Text,
    ),
    remote_property(ctx, found.remote().name().as_str(), &found.repo().to_string()),
  ];
  if let Some(view) = view.filter(|_| matches!(state, DetectionState::Detected { .. })) {
    rows.push(styled_property(
      &ctx.t("github.detection.property.defaultBranch"),
      view.default_branch.as_str(),
      ValueStyle::Mono,
    ));
  }
  if let (DetectionState::WrongAccount { .. }, Some(expected)) = (state, expected) {
    rows.push(styled_property(
      &ctx.t("github.detection.property.expectedAccount"),
      expected.as_str(),
      ValueStyle::Text,
    ));
  }
  rows.push(account_property(ctx, found, state));
  rows
}

fn account_property(ctx: &Ctx, found: &GitHubDetection, state: &DetectionState) -> Element {
  let host = found.repo().host().as_str();
  let label = ctx.t("github.detection.property.account");
  let known = |account: &GitHubLogin| {
    ctx.t_args(
      "github.detection.value.account",
      [("account", account.as_str()), ("host", host)],
    )
  };
  match (state, found.account()) {
    (DetectionState::GhNotFound, _) => styled_property(
      &label,
      &ctx.t("github.detection.value.unknownUntilGh"),
      ValueStyle::Pending,
    ),
    (DetectionState::NotSignedIn, _) => {
      styled_property(&label, &ctx.t("github.connection.notSignedIn"), ValueStyle::Pending)
    }
    (DetectionState::MissingScope { account }, _) => {
      let scopes = match found.check().map(|check| &check.result) {
        Some(GitHubCheckResult::Available { scopes, .. }) => scopes.join(", "),
        _ => String::new(),
      };
      let value = ctx.t_args(
        "github.detection.value.accountScopes",
        [("account", account.as_str()), ("scopes", scopes.as_str())],
      );
      styled_property(&label, &value, ValueStyle::Text)
    }
    (_, Some(account)) => styled_property(&label, &known(account), ValueStyle::Text),
    (_, None) => styled_property(&label, &ctx.t("github.connection.unknown"), ValueStyle::Pending),
  }
}

fn remote_property(ctx: &Ctx, remote: &str, address: &str) -> Element {
  let value = ctx.t_args(
    "github.detection.value.remote",
    [("remote", remote), ("address", address)],
  );
  styled_property(&ctx.t("github.detection.property.remote"), &value, ValueStyle::Mono)
}

fn repository_name(ctx: &Ctx, repo: &GitHubRepo) -> Arc<str> {
  ctx.t_args(
    "github.connection.repositoryName",
    [("owner", repo.owner()), ("name", repo.name())],
  )
}
