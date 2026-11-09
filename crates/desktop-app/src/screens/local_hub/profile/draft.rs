//! A local profile's draft in its panel (the team hub's edit parts, `Agents · MCP server · admin · draft` A13 `WMtxm`,
//! over the profile's own store): once a field is changed the panel says *Draft on v4 · saved 16:40 · only on this
//! computer* under the chips, and its pinned bar is *Publish v5* and *Discard draft*; discarding asks first, in a modal.

use std::{path::PathBuf, sync::Arc};

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::{
  id::ResourceId,
  resource::{AgentProfile, VersionNumber},
};
use orchester_workspace::{
  agents::{editor, local_hub::LocalProfile},
  display, removal,
  tasks::outside::Outside,
};

use super::super::model::Entry;
use crate::{
  project::ShownProject,
  screens::{
    hub::{
      discard::{self, DiscardProps},
      draft_bar::{DraftBar, Standing},
    },
    team_hub::{actions::Busy, bar},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
  },
};

/// The version after `number`, as the words say it (*v5*).
fn next(number: VersionNumber) -> String {
  number.next().map_or_else(String::new, |next| next.to_string())
}

/// *Draft on v4 · saved 16:40 · only on this computer* over what publishing makes of it.
pub(super) fn note(ctx: &Ctx, profile: &LocalProfile, entry: &Entry) -> Option<Element> {
  let saved = profile.draft_saved_at?;
  let published = entry.number?;
  let status = ctx.t_args(
    "local_hub.draft.saved",
    [("version", published.to_string()), ("time", display::clock(saved))],
  );
  let sub = ctx.t_args(
    "local_hub.draft.sub",
    [("next", next(published)), ("name", entry.name.clone())],
  );
  Some(
    DraftBar::new("local-hub-profile-draft", Standing::Draft, status)
      .sub(sub)
      .into(),
  )
}

/// The pinned bar of a draft: *Publish v5* and *Discard draft*, which `presses` do. The publication is loading while it
/// is taken; both wait while the hub is read again.
pub(super) fn bar(ctx: &Ctx, entry: &Entry, presses: (Press, Press), busy: Busy) -> Element {
  let (publish, discard) = presses;
  let label = ctx.t_args(
    "local_hub.draft.publish",
    [("version", entry.number.map_or_else(String::new, next))],
  );
  let publishing = ActionButton::new("local-hub-profile-publish", ButtonKind::Primary, label.to_string())
    .icon(Icon::Upload)
    .pinned();
  let discarding = ActionButton::new(
    "local-hub-profile-discard",
    ButtonKind::Secondary,
    ctx.t("local_hub.draft.discard").to_string(),
  )
  .icon(Icon::X)
  .pinned();
  let (publishing, discarding) = match busy {
    Busy::Idle => (publishing.press(publish), discarding.press(discard)),
    Busy::Taking => (publishing.loading(), discarding.disabled()),
    Busy::Reading => (publishing.disabled(), discarding.disabled()),
  };
  bar::bar(
    vec![publishing.into(), discarding.into()],
    Some(ctx.t("local_hub.draft.hint").to_string()),
  )
}

/// *Discard draft v5?* over the page while `open` holds: the draft goes only while it is still the one the panel
/// shows, and the selects then show the version again (`settled` counts the writes that ended).
pub(super) fn discard_question(
  ctx: &mut Ctx,
  (project, id, entry): (&ShownProject, ResourceId, &Entry),
  (open, settled): (&Signal<bool>, &Signal<u64>),
) -> Element {
  let root: PathBuf = project.open.repo_root.clone();
  let published = entry.number.map_or(0, VersionNumber::get);
  let (keep, done, settled) = (open.clone(), open.clone(), settled.clone());
  discard::question(
    ctx,
    DiscardProps {
      project: project.clone(),
      versions: (published + 1, published),
      discard: Arc::new(move || {
        let draft = editor::entry_in(&root, id)?;
        removal::discard_in::<AgentProfile>(&root, id, &draft.profile)
      }),
      change: Outside::Profiles,
      on_keep: Arc::new(move || keep.set(false)),
      on_discarded: Arc::new(move || {
        done.set(false);
        settled.update(|count| *count += 1);
      }),
    },
  )
}
