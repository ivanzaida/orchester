//! The panel of a team profile (`Agents · Profile` `NDZXC` C1, `WPvZx` C2, `QOBF2` C3; `Agents · All · admin`
//! `t4mtQL`; `docs/design/team-configuration.md`, "What a computer fills in"): its head and chips, *From the team*
//! (what every computer gets), *On this computer* (the connection it runs on: chosen without asking, a pick among
//! several, or none to add), *Ready here?* (each part it needs here, with the fix of what is missing) and, pinned at
//! the bottom, the fix of the first missing part.
//!
//! The fixes are the person's: they write through the fill-ins of `orchester-workspace`
//! ([`pick_team_connection`](orchester_workspace::workspace::Workspace::pick_team_connection), the program path, which
//! MCP servers are on, a secret's value). The connection is picked by a person or by automation as the workspace
//! takes it; the others widen what agents may do, so a press that automation made does nothing and says so.

mod bar;
mod computer;
mod edit;
pub(in crate::screens) mod fix;
pub(in crate::screens) mod from_team;
pub(in crate::screens) mod parts;
pub(in crate::screens) mod ready;
mod role_read;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::{ConnectionId, ProjectId, ResourceId};
use orchester_workspace::{
  agents::team_profile::{ConnectionChoice, TeamProfileSheet},
  messages::Message,
  sharing::team::TeamConfiguration,
};

use self::fix::Fix;
use super::{
  actions::Actions,
  archive, member,
  model::{Entry, Listing},
  panel, secret, words,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, HubFilter, HubView, ProjectPage, Route},
  screens::settings::orchester::PersonOnly,
  services::Services,
  theme::HubSpacing,
  ui::{field::field_error, message::message_text, pressable::Press},
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The person is an admin of the team, reachable or not: they edit it in place, and what they typed stays while the
  /// server is lost.
  pub admin: bool,
  /// Why the team server cannot be reached now; Save waits for it.
  pub unreached: Option<Message>,
  /// The team's configuration: an admin may change its resources, and a member reads who the admins are.
  pub team: TeamConfiguration,
  /// The hub is read again: what shows is the last answer, and a fix would act on it.
  pub reading: bool,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: TeamProfileSheet,
}
impl DevtoolsInspectable for Props {}

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<ProfilePanel>(&format!("team-hub-profile-{}", props.entry.id), props)
}

/// The panel of one profile: it is keyed by the resource, so a pick, an open dialog or a typed value never carries
/// over to another profile.
struct ProfilePanel {
  /// The connection the person chose in the select, by id; empty while none is chosen.
  chosen: Signal<String>,
  /// The connection kept as the profile's on this computer, by id; empty while none is. Once there is one, choosing
  /// another in the select keeps that one.
  kept: Signal<String>,
  /// The pick the sheet last held, by id; a different one is a pick made elsewhere, which the select then follows.
  seen: Signal<String>,
  actions: Actions,
  secret: secret::Entry,
  person: PersonOnly,
  /// An admin's edit of what the team holds of it.
  edit: Option<edit::ProfileEdit>,
}

impl Component for ProfilePanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let picked = props.sheet.picked.as_ref().map(ToString::to_string).unwrap_or_default();
    let panel = Self {
      chosen: ctx.signal(picked.clone()),
      kept: ctx.signal(picked.clone()),
      seen: ctx.signal(picked),
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
      edit: (props.admin && props.sheet.edit.is_some()).then(|| {
        edit::ProfileEdit::new(
          ctx,
          (props.repo_root.clone(), props.project, props.entry.id),
          &props.sheet,
        )
      }),
    };
    panel.watch_pick(ctx, props.entry.id);
    panel.secret.follow_save(ctx, &panel.actions.add_value);
    panel
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Props {
      listing, entry, sheet, ..
    } = &props;
    self.follow_pick(sheet);
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    let parts = parts::parts(ctx, listing, entry, sheet);
    let navigation = Navigation::of(ctx);
    let busy = self.actions.busy(props.reading);
    let press = |fix: &Fix| self.press(fix, &props, &navigation);
    let connection = self.connection(sheet);
    let words = bar::Words {
      listing,
      entry,
      sheet,
      connection: &connection,
    };
    let head_end = archive::menu(ctx, &props.repo_root, entry, props.team.may_change);
    let team = self.team_part(ctx, &props);
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Sections.role(),
        HubSpacing::BodyTop.role(),
      ))
      .spacing(HubSpacing::BodyTop.role())
      .child(panel::head(ctx, entry, head_end))
      .child(team.chips)
      .with_children(team.notice)
      .with_children(member::lock_line(ctx, &props.team).filter(|_| self.edit.is_none()))
      .with_children(team.sections)
      .child(computer::section(ctx, sheet, &self.chosen))
      .child(ready::section(
        ctx,
        (&ctx.t("team_hub.readiness.title"), &parts),
        missing.len() + usize::from(fix::archived_named(entry).is_some()),
        (&press, busy),
      ))
      .with_children(self.failure(ctx));
    let scroll = ScrollVertical::new(body)
      .id("team-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    let pinned = team
      .bar
      .or_else(|| fix::first(entry).map(|fix| bar::bar(ctx, &words, (&fix, missing), press(&fix), busy)));
    let dialog = self.secret.dialog(
      ctx,
      secret::Entry::save_by(&self.actions.add_value, &self.person),
      self.actions.add_value.state().get().is_pending(),
    );
    Column::new()
      .id("team-hub-panel")
      .describe("resource", entry.id.to_string())
      .describe("kind", entry.kind.name())
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(scroll)
      .with_children(pinned)
      .with_children(dialog)
  }
}

impl Drop for ProfilePanel {
  fn drop(&mut self) {
    self.secret.wipe();
  }
}

/// The connection the select holds (by id), else the first one listed: the one *Use* names and keeps.
fn chosen_connection<'a>(chosen: &str, connections: &'a [ConnectionChoice]) -> Option<&'a ConnectionChoice> {
  connections
    .iter()
    .find(|choice| choice.id.to_string() == chosen)
    .or_else(|| connections.first())
}

impl ProfilePanel {
  /// What the team holds of it: an admin's edit (A26), else the plain *From the team*.
  fn team_part(&self, ctx: &mut Ctx, props: &Props) -> TeamPart {
    let config = Services::of(ctx)
      .workspace
      .team_read(&props.repo_root)
      .config()
      .cloned();
    let drawn = self
      .edit
      .as_ref()
      .and_then(|edit| edit.draw(ctx, (&props.sheet, config.as_deref()), props.unreached.as_ref()));
    let Some(edit::Drawn { parts, sections }) = drawn else {
      return TeamPart {
        chips: panel::chips(ctx, &props.listing, &props.entry),
        notice: None,
        sections: role_read::section(ctx, &props.sheet)
          .into_iter()
          .chain([from_team::section(ctx, &props.sheet, super::rows::FromTeam::Shared).into()])
          .collect(),
        bar: None,
      };
    };
    let badges = parts
      .chip
      .into_iter()
      .chain([words::chip(ctx, &props.listing, &props.entry)])
      .collect();
    // The fix of what this computer lacks stays pinned while nothing is edited; Save's bar takes its place once
    // something is.
    let fix = fix::first(&props.entry);
    TeamPart {
      chips: panel::chips_of(ctx, &props.entry, badges, None),
      notice: parts.notice,
      sections,
      bar: (parts.holds || fix.is_none()).then_some(parts.bar),
    }
  }

  /// Once a connection is kept, choosing another in the select keeps that one (as *Use* does, and as the MCP tool
  /// does for automation); before, the select only chooses and *Use* keeps it. A pick that failed puts the select
  /// back on the one kept.
  fn watch_pick(&self, ctx: &mut Ctx, profile: ResourceId) {
    let (kept, pick) = (self.kept.clone(), self.actions.pick.clone());
    ctx.watch(&self.chosen, move |now| {
      let kept = kept.get();
      if kept.is_empty() || *now == kept {
        return;
      }
      if let Ok(connection) = now.parse::<ConnectionId>() {
        pick.run((profile, connection));
      }
    });
    let (chosen, kept) = (self.chosen.clone(), self.kept.clone());
    ctx.watch(&self.actions.pick.state(), move |state| {
      if state.is_fulfilled() {
        kept.set(chosen.get());
      } else if state.error.is_some() && !kept.get().is_empty() {
        chosen.set(kept.get());
      }
    });
  }

  /// A connection picked outside the panel (an MCP client's `pick_profile_connection`) shows in the sheet's `picked`:
  /// the select and the one kept follow it, without asking the workspace to pick it again.
  fn follow_pick(&self, sheet: &TeamProfileSheet) {
    let picked = sheet.picked.as_ref().map(ToString::to_string).unwrap_or_default();
    if self.seen.get_untracked() == picked {
      return;
    }
    self.seen.set(picked.clone());
    self.kept.set(picked.clone());
    self.chosen.set(picked);
  }

  /// The name of the connection the pinned *Use* names and uses.
  fn connection(&self, sheet: &TeamProfileSheet) -> String {
    chosen_connection(&self.chosen.get(), &sheet.connections)
      .map(|choice| choice.name.clone())
      .unwrap_or_default()
  }

  /// What a press on the fix `fix` does, for the control `id`.
  fn press(&self, fix: &Fix, props: &Props, navigation: &Navigation) -> Press {
    let person = self.person.clone();
    let guarded = |action: Arc<dyn Fn() + Send + Sync>| {
      Press::run(move || {
        if person.allows("fix what a team profile needs on this computer") {
          action();
        }
      })
    };
    match fix {
      Fix::AddConnection | Fix::SignIn => Press::Open(navigation.to(Route::Accounts(AccountsView::default()))),
      Fix::OpenArchived(resource) => Press::Open(navigation.to(Route::Project(
        props.project,
        ProjectPage::hub(HubView::new(HubFilter::Archived, Some(*resource))),
      ))),
      Fix::UseConnection => {
        let (action, profile, chosen) = (self.actions.pick.clone(), props.entry.id, self.chosen.clone());
        let connections = props.sheet.connections.clone();
        Press::run(move || {
          if let Some(choice) = chosen_connection(&chosen.get(), &connections) {
            // The select shows the one kept; with none chosen yet that is the first listed.
            chosen.set(choice.id.to_string());
            action.run((profile, choice.id));
          }
        })
      }
      Fix::SwitchOn(server) => {
        let (action, server) = (self.actions.switch_on.clone(), *server);
        guarded(Arc::new(move || action.run(server)))
      }
      Fix::SetPath(server) => {
        let (action, server) = (self.actions.set_path.clone(), *server);
        guarded(Arc::new(move || action.run(server)))
      }
      Fix::AddValue(name) => {
        let (entry, name) = (self.secret.clone(), name.clone());
        guarded(Arc::new(move || entry.ask(&name)))
      }
    }
  }

  /// Why the last fix did nothing: what the workspace refused with. What went wrong with a secret's value shows in its
  /// dialog.
  fn failure(&self, ctx: &Ctx) -> Option<Element> {
    let reason = self.actions.refused()?;
    Some(field_error("team-hub-fix-error", &message_text(ctx, &reason)).into())
  }
}

/// What the panel shows of what the team holds of the profile.
struct TeamPart {
  chips: lurq::components::Row,
  /// What the last Save of an edit was answered, or that the team server cannot be reached while changes wait.
  notice: Option<Element>,
  sections: Vec<Element>,
  /// An edit's Save/Cancel bar, pinned in the place of the fix.
  bar: Option<Element>,
}
