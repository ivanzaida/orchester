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
mod rows;
mod secret;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::{Column, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId},
  run::Answerer,
  secret::{SecretName, SecretValue},
};
use orchester_store::ConnectionPick;
use orchester_workspace::{
  agents::team_profile::{ConnectionChoice, TeamProfileSheet},
  background,
  messages::Message,
};

use self::fix::Fix;
use super::{
  model::{Entry, Listing},
  panel,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::settings::orchester::PersonOnly,
  services::Services,
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

/// The id the person-only check of a fix is made under, and of the dialog's *Save*.
const FIX: &str = "team-hub-fix";
const SAVE: &str = "team-hub-secret-save";

pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<ProfilePanel>(&format!("team-hub-profile-{}", props.entry.id), props)
}

struct ProfilePanel {
  /// The connection the person chose in the select, by id; empty while none is chosen.
  chosen: Signal<String>,
  pick: FutureAction<(ResourceId, ConnectionId), (), Message>,
  set_path: FutureAction<ResourceId, (), Message>,
  switch_on: FutureAction<ResourceId, (), Message>,
  add_value: FutureAction<(SecretName, String), (), Message>,
  secret: secret::Entry,
  person: PersonOnly,
}

impl Component for ProfilePanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace, services.vault);
    let root = props.repo_root;
    let pick = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |(profile, connection): (ResourceId, ConnectionId)| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          let pick = ConnectionPick {
            connection,
            account: None,
          };
          background::run(move || workspace.pick_team_connection(&root, profile, pick))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let set_path = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |server: ResourceId| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          let Some(program) = background::run(|| rfd::FileDialog::new().pick_file()).await? else {
            return Ok(());
          };
          background::run(move || workspace.set_team_mcp_program(&root, server, &program, Answerer::Person))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let switch_on = {
      let (workspace, root) = (workspace.clone(), root.clone());
      ctx.future_action(move |server: ResourceId| {
        let (workspace, root) = (workspace.clone(), root.clone());
        async move {
          background::run(move || workspace.switch_team_mcp_on_here(&root, server, true, Answerer::Person))
            .await?
            .map_err(|error| error.message())
        }
      })
    };
    let add_value = ctx.future_action(move |(name, value): (SecretName, String)| {
      let (workspace, root, vault) = (workspace.clone(), root.clone(), vault.clone());
      async move {
        let value = SecretValue::new(value).map_err(|_| Message::new("teamFillIns.errors.valueBlank"))?;
        background::run(move || workspace.set_team_secret_value(&root, vault.get(), &name, &value))
          .await?
          .map_err(|error| error.message())
      }
    });
    Self {
      chosen: ctx.signal(props.sheet.picked.map(|picked| picked.to_string()).unwrap_or_default()),
      pick,
      set_path,
      switch_on,
      add_value,
      secret: secret::Entry::new(ctx),
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Props {
      listing, entry, sheet, ..
    } = &props;
    let missing = entry.standing.as_ref().map_or(&[][..], |standing| standing.missing());
    let parts = parts::parts(ctx, listing, entry, sheet);
    let navigation = Navigation::of(ctx);
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
      .child(ready::section(ctx, &parts, missing.len(), &press))
      .with_children(self.failure(ctx));
    let scroll = ScrollVertical::new(body)
      .id("team-hub-panel-scroll")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0));
    let pinned = missing
      .first()
      .and_then(Fix::of)
      .map(|fix| bar::bar(ctx, &words, (&fix, missing), press(&fix), self.pending()));
    let dialog = self
      .secret
      .dialog(ctx, self.save_value(), self.add_value.state().get().is_pending());
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

/// The connection the select holds (by id), else the first one listed: the one *Use* names and keeps.
fn chosen_connection<'a>(chosen: &str, connections: &'a [ConnectionChoice]) -> Option<&'a ConnectionChoice> {
  connections
    .iter()
    .find(|choice| choice.id.to_string() == chosen)
    .or_else(|| connections.first())
}

impl ProfilePanel {
  /// The name of the connection the pinned *Use* names and uses.
  fn connection(&self, sheet: &TeamProfileSheet) -> String {
    chosen_connection(&self.chosen.get(), &sheet.connections)
      .map(|choice| choice.name.clone())
      .unwrap_or_default()
  }

  fn pending(&self) -> bool {
    self.pick.state().get().is_pending()
      || self.set_path.state().get().is_pending()
      || self.switch_on.state().get().is_pending()
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
        let (action, profile, chosen) = (self.pick.clone(), props.entry.id, self.chosen.clone());
        let connections = props.sheet.connections.clone();
        Press::run(move || {
          if let Some(choice) = chosen_connection(&chosen.get(), &connections) {
            action.run((profile, choice.id));
          }
        })
      }
      Fix::SwitchOn(server) => {
        let (action, server) = (self.switch_on.clone(), *server);
        guarded(Arc::new(move || action.run(server)))
      }
      Fix::SetPath(server) => {
        let (action, server) = (self.set_path.clone(), *server);
        guarded(Arc::new(move || action.run(server)))
      }
      Fix::AddValue(name) => {
        let (entry, name) = (self.secret.clone(), name.clone());
        guarded(Arc::new(move || entry.ask(&name)))
      }
    }
  }

  /// What the dialog's *Save* does, a person's alone.
  fn save_value(&self) -> Arc<dyn Fn(SecretName, String) + Send + Sync> {
    let (action, person) = (self.add_value.clone(), self.person.clone());
    Arc::new(move |name, value| {
      if person.allows(SAVE) {
        action.run((name, value));
      }
    })
  }

  /// *Edit* in the head, which an admin has where the kind has an editor.
  fn edit(&self, ctx: &Ctx, props: &Props, navigation: &Navigation) -> Option<Element> {
    let route = panel::editor(&props.entry, props.project).filter(|_| props.may_change)?;
    Some(panel::edit_button(ctx, navigation.to(route)))
  }

  /// Why the last fix did nothing: what the workspace refused with, or that automation pressed it.
  fn failure(&self, ctx: &Ctx) -> Option<Element> {
    let reason = [
      self.pick.state().get(),
      self.set_path.state().get(),
      self.switch_on.state().get(),
      self.add_value.state().get(),
    ]
    .into_iter()
    .find_map(|state| state.error.clone().filter(|_| !state.is_pending()));
    if let Some(reason) = reason {
      return Some(field_error("team-hub-fix-error", &message_text(ctx, &reason)).into());
    }
    [FIX, SAVE].iter().find_map(|id| self.person.reason(ctx, id))
  }
}
