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
mod fix;
mod from_team;
mod parts;
mod ready;

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
use orchester_workspace::agents::team_profile::{ConnectionChoice, TeamProfileSheet};

use self::fix::Fix;
use super::{
  actions::Actions,
  model::{Entry, Listing},
  panel, secret,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::settings::orchester::PersonOnly,
  theme::HubSpacing,
  ui::{field::field_error, message::message_text, pressable::Press},
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub repo_root: PathBuf,
  pub project: ProjectId,
  /// The person may change the team's resources: an admin of the team.
  pub may_change: bool,
  /// The hub is read again: what shows is the last answer, and a fix would act on it.
  pub reading: bool,
  pub listing: Listing,
  pub entry: Entry,
  pub sheet: TeamProfileSheet,
}
impl DevtoolsInspectable for Props {}

/// The id the person-only check of a fix is made under.
const FIX: &str = "team-hub-fix";

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
  actions: Actions,
  secret: secret::Entry,
  person: PersonOnly,
}

impl Component for ProfilePanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let picked = props.sheet.picked.as_ref().map(ToString::to_string).unwrap_or_default();
    let panel = Self {
      chosen: ctx.signal(picked.clone()),
      kept: ctx.signal(picked),
      actions: Actions::new(ctx, &props.repo_root),
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
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
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        HubSpacing::Sections.role(),
        HubSpacing::BodyTop.role(),
      ))
      .spacing(HubSpacing::BodyTop.role())
      .child(panel::head(ctx, entry, self.edit(ctx, &props, &navigation)))
      .child(panel::chips(ctx, listing, entry))
      .child(from_team::section(ctx, sheet))
      .child(computer::section(ctx, sheet, &self.chosen))
      .child(ready::section(ctx, &parts, missing.len(), (&press, busy)))
      .with_children(self.failure(ctx));
    let scroll = ScrollVertical::new(body)
      .id("team-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    let pinned = missing
      .first()
      .and_then(Fix::of)
      .map(|fix| bar::bar(ctx, &words, (&fix, missing), press(&fix), busy));
    let under = self.person.reason(ctx, secret::SAVE);
    let dialog = self.secret.dialog(
      ctx,
      secret::Entry::save_by(&self.actions.add_value, &self.person),
      self.actions.add_value.state().get().is_pending(),
      under,
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
        if person.allows(FIX) {
          action();
        }
      })
    };
    match fix {
      Fix::AddConnection => Press::Open(navigation.to(Route::Accounts(AccountsView::default()))),
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

  /// *Edit* in the head, which an admin has where the kind has an editor.
  fn edit(&self, ctx: &Ctx, props: &Props, navigation: &Navigation) -> Option<Element> {
    let route = panel::editor(&props.entry, props.project).filter(|_| props.may_change)?;
    Some(panel::edit_button(ctx, navigation.to(route)))
  }

  /// Why the last fix did nothing: what the workspace refused with, or that automation pressed it. What went wrong
  /// with a secret's value shows in its dialog.
  fn failure(&self, ctx: &Ctx) -> Option<Element> {
    if let Some(reason) = self.actions.refused() {
      return Some(field_error("team-hub-fix-error", &message_text(ctx, &reason)).into());
    }
    self.person.reason(ctx, FIX)
  }
}
