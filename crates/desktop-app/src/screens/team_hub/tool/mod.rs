//! The panel of a team tool (`Agents · Tools` `u9Hcc` A6, `yTVeE` M6; `docs/design/team-configuration.md`, "What a
//! computer fills in"): its head and chips, *From the team* (what it runs or calls, its arguments, who may call it
//! without asking and the secret it names, the same on every computer), *On this computer* (how it stands) and, pinned
//! at the bottom, the admin's *Edit*.
//!
//! A team tool names its secret by id and has no team form for a name yet (`Undecided::TeamToolSecretById`), so there
//! is nothing to fill in for it here: the panel says so and offers no action, and holds no state of its own.

mod computer;
mod from_team;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  agents::team_tool::TeamToolSheet,
  readiness::{ReadinessStanding, Undecided},
  sharing::team::TeamConfiguration,
};

use super::{
  archive, bar, member,
  model::{Entry, Listing},
  panel, words,
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, ToolTab},
  services::Services,
  ui::chip::BadgeTone,
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The team's configuration: an admin may change its resources, and a member reads who the admins are.
  pub team: TeamConfiguration,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: TeamToolSheet,
}
impl DevtoolsInspectable for Props {}

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<ToolPanel>(&format!("team-hub-tool-{}", props.entry.id), props)
}

/// The panel of one tool, keyed by the resource so its scroll never carries over to another.
struct ToolPanel;

impl Component for ToolPanel {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      repo_root,
      project,
      team,
      listing,
      entry,
      sheet,
    } = ctx.props::<Props>().clone();
    let now = Services::of(ctx).clock.now();
    let navigation = Navigation::of(ctx);
    let route = Route::Project(project, ProjectPage::Tool(entry.id, ToolTab::default()));
    let head_edit = panel::with_more(
      team
        .may_change
        .then(|| panel::edit_button(ctx, navigation.to(route.clone()))),
      archive::menu(ctx, &repo_root, &entry, team.may_change),
    );
    let kind = ctx.t_args(
      "team_hub.tool.where",
      [("kind", from_team::kind(ctx, &sheet).to_string())],
    );
    let version = words::published_line(ctx, now, &entry, sheet.published.as_ref());
    let chips = panel::chips_saying(
      ctx,
      &entry,
      chip(ctx, &listing, &entry),
      panel::used_by(ctx, &sheet.used_by),
    );
    let body = panel::body()
      .child(panel::head_of(&entry, (kind.to_string(), version), head_edit))
      .child(chips)
      .with_children(member::lock_line(ctx, &team))
      .child(from_team::section(ctx, &sheet))
      .child(computer::section(ctx, &entry, &sheet));
    let bar = team.may_change.then(|| bar::edit(ctx, &entry, navigation.to(route)));
    panel::frame(&entry, body, bar)
  }
}

/// The panel's readiness chip: the line of the list, but for a secret named by id, which the panel says in its own
/// words.
fn chip(ctx: &Ctx, listing: &Listing, entry: &Entry) -> (Arc<str>, BadgeTone) {
  match &entry.standing {
    Some(ReadinessStanding::Undecided(Undecided::TeamToolSecretById(_))) => {
      (ctx.t("team_hub.tool.chip.secretById"), BadgeTone::Warning)
    }
    _ => words::chip(ctx, listing, entry),
  }
}
