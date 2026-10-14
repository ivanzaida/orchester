//! What the GitHub block says in each detection state (`gh5wV`): its status, title, note and properties. The
//! states the board does not draw (git failed, gh not answering, unreachable, rate limited, an empty
//! repository) use the same parts.

use std::{path::Path, sync::Arc};

use lurq::app::{ctx::Ctx, theme::PaletteColor};
use orchester_core::{
  connection::{GitHubCheckResult, GitHubLogin, REQUIRED_SCOPE},
  repository::{Detection, DetectionState, GitHubDetection, GitHubRepo},
};
use orchester_workspace::display;

use crate::ui::icon::Icon;

/// Longest folder path shown in full in a property (`Value` in `gutA8`).
const PATH_CHARS: usize = 64;

/// The status under the title (`Status line` `yKHnp`): its icon, the icon's and the word's colors, and the
/// word's key; `None` while detecting.
pub(super) fn status(state: Option<&DetectionState>) -> (Icon, PaletteColor, PaletteColor, &'static str) {
  use PaletteColor::{Danger, Success, TextMuted, TextSecondary, Warning};
  let Some(state) = state else {
    return (
      Icon::LoaderCircle,
      TextSecondary,
      TextSecondary,
      "setup.github.status.checking",
    );
  };
  match state {
    DetectionState::Detected { .. } => (
      Icon::CircleCheck,
      Success,
      TextSecondary,
      "setup.github.status.detected",
    ),
    DetectionState::NoRemote => (
      Icon::CircleMinus,
      TextMuted,
      TextSecondary,
      "setup.github.status.localOnly",
    ),
    DetectionState::NotGitHub => (
      Icon::CircleMinus,
      TextMuted,
      TextSecondary,
      "setup.github.status.notGitHub",
    ),
    DetectionState::NoDefaultBranch => (
      Icon::CircleMinus,
      TextMuted,
      TextSecondary,
      "setup.github.status.emptyRepository",
    ),
    DetectionState::GhNotFound => (Icon::CircleAlert, Danger, Danger, "setup.github.status.ghNotFound"),
    DetectionState::NotSignedIn => (Icon::CircleAlert, Warning, Warning, "setup.github.status.notSignedIn"),
    DetectionState::WrongAccount { .. } => (Icon::CircleAlert, Warning, Warning, "setup.github.status.wrongAccount"),
    DetectionState::NoAccess { .. } => (Icon::Lock, Warning, Warning, "setup.github.status.noAccess"),
    DetectionState::MissingScope { .. } => (Icon::KeyRound, Warning, Warning, "setup.github.status.missingScope"),
    DetectionState::Unrecognized => (Icon::CircleHelp, Danger, Danger, "setup.github.status.unrecognized"),
    DetectionState::NotAGitRepository => (Icon::CircleX, Danger, Danger, "setup.github.status.notRepository"),
    DetectionState::GitFailed => (Icon::CircleAlert, Danger, Danger, "setup.github.status.gitFailed"),
    DetectionState::CouldNotRun => (Icon::CircleAlert, Danger, Danger, "setup.github.status.couldNotRun"),
    DetectionState::Unreachable => (Icon::CircleAlert, Warning, Warning, "setup.github.status.unreachable"),
    DetectionState::RateLimited => (Icon::Clock, Warning, Warning, "setup.github.status.rateLimited"),
  }
}

/// The block's title (`Title` `yFgXP`).
pub(super) fn heading(ctx: &Ctx, state: Option<&DetectionState>, found: Option<&GitHubDetection>) -> Arc<str> {
  let host = found.map_or_else(String::new, |found| found.repo().host().to_string());
  let Some(state) = state else {
    return ctx.t("setup.github.title");
  };
  match state {
    DetectionState::Detected { .. } => ctx.t("setup.github.title"),
    DetectionState::NoRemote => ctx.t("setup.github.heading.noRemote"),
    DetectionState::NotGitHub => ctx.t("setup.github.heading.notGitHub"),
    DetectionState::GhNotFound => ctx.t("setup.github.heading.ghNotFound"),
    DetectionState::NotSignedIn => ctx.t_args("setup.github.heading.notSignedIn", [("host", host)]),
    DetectionState::WrongAccount { active, expected } => ctx.t_args(
      "setup.github.heading.wrongAccount",
      [("active", active.as_str()), ("expected", expected.as_str())],
    ),
    DetectionState::NoAccess { account } => {
      ctx.t_args("setup.github.heading.noAccess", [("account", account.as_str())])
    }
    DetectionState::Unrecognized => ctx.t("setup.github.heading.unrecognized"),
    DetectionState::NotAGitRepository => ctx.t("setup.github.heading.notRepository"),
    DetectionState::MissingScope { .. } => ctx.t_args("setup.github.heading.missingScope", [("scope", REQUIRED_SCOPE)]),
    DetectionState::GitFailed => ctx.t("setup.github.heading.gitFailed"),
    DetectionState::CouldNotRun => ctx.t("setup.github.heading.couldNotRun"),
    DetectionState::Unreachable => ctx.t_args("setup.github.heading.unreachable", [("host", host)]),
    DetectionState::RateLimited => ctx.t("setup.github.heading.rateLimited"),
    DetectionState::NoDefaultBranch => ctx.t("setup.github.heading.emptyRepository"),
  }
}

/// What the state means and what to do (`Note` `k8bJb`); `project` is the name the project will have.
pub(super) fn note(
  ctx: &Ctx,
  state: Option<&DetectionState>,
  found: Option<&GitHubDetection>,
  project: &str,
) -> Arc<str> {
  let host = found.map_or_else(String::new, |found| found.repo().host().to_string());
  let Some(state) = state else {
    return ctx.t("setup.github.note.detecting");
  };
  match state {
    DetectionState::Detected { account } => ctx.t_args(
      "setup.github.note.detected",
      [("project", project), ("account", account.as_str())],
    ),
    DetectionState::NoRemote => ctx.t("setup.github.note.noRemote"),
    DetectionState::NotGitHub => ctx.t("setup.github.note.notGitHub"),
    DetectionState::GhNotFound => ctx.t("setup.github.note.ghNotFound"),
    DetectionState::NotSignedIn => ctx.t("setup.github.note.notSignedIn"),
    DetectionState::WrongAccount { active, expected } => ctx.t_args(
      "setup.github.note.wrongAccount",
      [
        ("project", project),
        ("active", active.as_str()),
        ("expected", expected.as_str()),
      ],
    ),
    DetectionState::NoAccess { .. } => ctx.t("setup.github.note.noAccess"),
    DetectionState::Unrecognized => match found.and_then(GitHubDetection::version) {
      Some(version) => ctx.t_args("setup.github.note.unrecognizedVersion", [("version", version)]),
      None => ctx.t("setup.github.note.unrecognized"),
    },
    DetectionState::NotAGitRepository => ctx.t("setup.github.note.notRepository"),
    DetectionState::MissingScope { account } => ctx.t_args(
      "setup.github.note.missingScope",
      [("account", account.as_str()), ("scope", REQUIRED_SCOPE)],
    ),
    DetectionState::GitFailed => ctx.t("setup.github.note.gitFailed"),
    DetectionState::CouldNotRun => ctx.t("setup.github.note.couldNotRun"),
    DetectionState::Unreachable => ctx.t_args("setup.github.note.unreachable", [("host", host)]),
    DetectionState::RateLimited => ctx.t("setup.github.note.rateLimited"),
    DetectionState::NoDefaultBranch => ctx.t("setup.github.note.emptyRepository"),
  }
}

/// How a property's value reads.
#[derive(Clone, PartialEq, Eq)]
pub(super) enum Value {
  /// Words in `text_primary`, or the color the state gives them.
  Text(PaletteColor),
  /// A folder, a remote, a host or scopes, in Geist Mono.
  Mono,
}

/// A property of the block (`Row · Repository` `Y7zqv`): what it is, its value and how the value reads.
pub(super) struct Property {
  pub label: Arc<str>,
  pub value: Arc<str>,
  pub look: Value,
}

/// What detection knows of the folder's repository, its remote and the account (`Properties` `Mcur1`).
pub(super) fn properties(
  ctx: &Ctx,
  detection: Option<&Detection>,
  state: Option<&DetectionState>,
  expected: Option<&GitHubLogin>,
  folder: &Path,
) -> Vec<Property> {
  let prop = |key: &str, value: Arc<str>, look| Property {
    label: ctx.t(&format!("setup.github.property.{key}")),
    value,
    look,
  };
  let (Some(detection), Some(state)) = (detection, state) else {
    let checking = ctx.t("setup.github.value.checking");
    return vec![
      prop("repository", checking.clone(), Value::Text(PaletteColor::TextMuted)),
      prop("account", checking, Value::Text(PaletteColor::TextMuted)),
    ];
  };
  match detection {
    Detection::NotAGitRepository | Detection::GitFailed(_) => {
      let mut rows = vec![prop(
        "folder",
        display::shorten_path(folder, PATH_CHARS).into(),
        Value::Mono,
      )];
      if matches!(state, DetectionState::NotAGitRepository) {
        rows.push(prop(
          "git",
          ctx.t("setup.github.value.noGitRepository"),
          Value::Text(PaletteColor::Danger),
        ));
      }
      rows
    }
    Detection::NoRemote(remote) => vec![prop(
      "remote",
      ctx.t_args("setup.github.value.noRemote", [("remote", remote.as_str())]),
      Value::Text(PaletteColor::TextSecondary),
    )],
    Detection::NotGitHub(remote) => vec![prop(
      "remote",
      remote_value(ctx, remote.name().as_str(), display::remote_address(remote.url())),
      Value::Mono,
    )],
    Detection::GitHub(found) => github(ctx, found, state, expected)
      .into_iter()
      .map(|(key, value, look)| prop(key, value, look))
      .collect(),
  }
}

/// The properties of a remote on GitHub, as the state shows them.
fn github(
  ctx: &Ctx,
  found: &GitHubDetection,
  state: &DetectionState,
  expected: Option<&GitHubLogin>,
) -> Vec<(&'static str, Arc<str>, Value)> {
  let primary = || Value::Text(PaletteColor::TextPrimary);
  let repository = (
    "repository",
    match found.view() {
      Some(view) => repository_name(ctx, &view.repo),
      None => ctx.t_args(
        "setup.github.value.fromRemote",
        [("owner", found.repo().owner()), ("name", found.repo().name())],
      ),
    },
    primary(),
  );
  let remote = (
    "remote",
    remote_value(ctx, found.remote().name().as_str(), &found.repo().to_string()),
    Value::Mono,
  );
  let account = ("account", account(ctx, found, state), account_look(state));
  match state {
    DetectionState::Detected { .. } => {
      let branch = found.view().map(|view| view.default_branch.as_str().into());
      let mut rows = vec![repository, remote];
      rows.extend(branch.map(|branch| ("defaultBranch", branch, Value::Mono)));
      rows.push(account);
      rows
    }
    DetectionState::WrongAccount { .. } => {
      let expected = expected.map(|expected| ("expectedAccount", expected.as_str().into(), primary()));
      std::iter::once(account).chain(expected).collect()
    }
    DetectionState::MissingScope { .. } => vec![account],
    DetectionState::GhNotFound
    | DetectionState::NotSignedIn
    | DetectionState::NoAccess { .. }
    | DetectionState::Unrecognized => vec![repository, account],
    _ => vec![repository, remote, account],
  }
}

/// The account `gh` acts as, or why it is not known.
fn account(ctx: &Ctx, found: &GitHubDetection, state: &DetectionState) -> Arc<str> {
  let host = found.repo().host().to_string();
  match (state, found.account()) {
    (DetectionState::GhNotFound, _) => ctx.t("setup.github.value.unknownUntilGh"),
    (DetectionState::NotSignedIn, _) => ctx.t("setup.github.value.notSignedIn"),
    (DetectionState::MissingScope { account }, _) => {
      let scopes = match found.check().map(|check| &check.result) {
        Some(GitHubCheckResult::Available { scopes, .. }) => scopes.join(", "),
        _ => String::new(),
      };
      ctx.t_args(
        "setup.github.value.accountScopes",
        [("account", account.as_str().to_owned()), ("scopes", scopes)],
      )
    }
    (_, Some(account)) => ctx.t_args(
      "setup.github.value.account",
      [("account", account.as_str().to_owned()), ("host", host)],
    ),
    (_, None) => ctx.t("setup.github.value.unknown"),
  }
}

/// The account's color: what is not known is `text_muted`, what is wrong `warning`.
fn account_look(state: &DetectionState) -> Value {
  Value::Text(match state {
    DetectionState::GhNotFound | DetectionState::Unrecognized | DetectionState::CouldNotRun => PaletteColor::TextMuted,
    DetectionState::NotSignedIn | DetectionState::WrongAccount { .. } => PaletteColor::Warning,
    _ => PaletteColor::TextPrimary,
  })
}

fn remote_value(ctx: &Ctx, remote: &str, address: &str) -> Arc<str> {
  ctx.t_args("setup.github.value.remote", [("remote", remote), ("address", address)])
}

pub(super) fn repository_name(ctx: &Ctx, repo: &GitHubRepo) -> Arc<str> {
  ctx.t_args(
    "setup.github.value.repository",
    [("owner", repo.owner()), ("name", repo.name())],
  )
}
