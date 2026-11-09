//! Where a service, a stack or a suite stands, as its page's last line says it beside *Publish vN* (`Actions` `doe9q`):
//! not saved yet, a draft, a draft from an older version, or which version is published.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::resource::VersionNumber;
use orchester_workspace::agents::data::VersionState;

/// Where it stands as stored; `None` for one not saved yet.
pub(super) fn state_words(ctx: &Ctx, state: Option<VersionState>) -> Arc<str> {
  let Some(state) = state else {
    return ctx.t("project_settings.services.editor.state.new");
  };
  let version = |number: VersionNumber| number.get().to_string();
  match state {
    VersionState::Draft { published: None } | VersionState::StaleDraft { published: None, .. } => {
      ctx.t("project_settings.services.editor.state.draftNew")
    }
    VersionState::Draft {
      published: Some(published),
    } => ctx.t_args(
      "project_settings.services.editor.state.draft",
      [("version", version(published))],
    ),
    VersionState::StaleDraft {
      based_on,
      published: Some(published),
    } => ctx.t_args(
      "project_settings.services.editor.state.stale",
      [
        ("based", based_on.map_or_else(String::new, version)),
        ("version", version(published)),
      ],
    ),
    VersionState::Published(published) => ctx.t_args(
      "project_settings.services.editor.state.published",
      [("version", version(published))],
    ),
  }
}

/// The version publishing makes next, while there is a draft or a change that will be one; `None` otherwise or past
/// the last number.
pub(super) fn next_version(state: VersionState, changed: bool) -> Option<VersionNumber> {
  if !state.has_draft() && !changed {
    return None;
  }
  state
    .published()
    .map_or(Some(VersionNumber::FIRST), VersionNumber::next)
}

/// Why Publish waits for a draft started from an older version, naming the version published since.
pub(super) fn stale_reason(ctx: &Ctx, state: VersionState) -> Option<Arc<str>> {
  let VersionState::StaleDraft {
    published: Some(published),
    ..
  } = state
  else {
    return None;
  };
  Some(ctx.t_args(
    "project_settings.services.editor.publishStale",
    [("version", published.get().to_string())],
  ))
}
