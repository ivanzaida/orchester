//! The panel of a local profile (`Agents · Profile` L1 `fFxoM`, `docs/design/team-configuration.md` rule 11): the head
//! with the version and its *Versions*, the chips (how it stands, who uses it), *Role* (its name, what it answers for
//! and the role instructions, L17 `b7Tbxb`: [`role`]), *Profile* (CLI, connection, model,
//! effort, MCP servers, secrets, and the instructions, skills and tools it gives its agents, which nothing else gives
//! them: `docs/design/team-configuration.md`, "Decided 2026-11-09"), *Ready?* with the fix of what is missing, and
//! the pinned *Save* and *Cancel* ([`super::save_bar`]). The parts are the team profile's
//! ([`crate::screens::team_hub::profile`]) over the local profile store. The profile is edited in place, as the design
//! draws it ([`table`], [`pick`], [`add_menu`]): the CLI, the connection, the model, the reasoning effort, the MCP
//! servers, instructions, skills and tools are selects, and each choice changes only what the panel holds, at once and
//! without locking anything it shows ([`choices`]). *Save* publishes it as the next
//! version at once ([`edit`]); *Cancel* puts back what is published.
//!
//! The panel has no drafts (decided by the person 2026-11-09): it opens on the published version, unchanged, even when
//! a draft of the profile is stored (an MCP client's), which *Save* replaces with what it publishes and *Cancel* leaves
//! alone.

pub(in crate::screens) mod add_menu;
mod choices;
mod edit;
mod pick;
mod ready;
mod role;
mod table;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::{Ref, Signal},
  node::Element,
};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  resource::AgentProfile,
};
use orchester_workspace::{
  agents::{
    local_edit::Edit,
    local_hub::{LocalHub, LocalProfile},
    role::RoleErrors,
  },
  local_archive::Archivable,
  messages::Message,
};

use self::{
  choices::Choices,
  edit::{Ask, Followed, Holding},
  ready::Fix,
};
use super::{
  leave::{clear_unsaved, mark_unsaved},
  mcp::UnsavedSignal,
  model::{Entry, view_of},
  panel::{chips_of, frame, head_end},
  save_bar::{self, Save},
  words::kind_where_key,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  project::ShownProject,
  routes::{AccountsView, HubFilter, ProjectPage, Route},
  screens::team_hub::{
    actions::Busy,
    panel::{body, head_named},
    profile::ready::section,
  },
  ui::{field::field_error, message::message_text, pressable::Press},
};

/// The ids of the panel's parts start with it.
const PREFIX: &str = "local-hub-profile";

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  pub id: ResourceId,
  pub profile: LocalProfile,
  pub hub: LocalHub,
  /// What the list says of edits that are not saved, which the panel keeps.
  pub unsaved: UnsavedSignal,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.project, &self.entry, self.id, &self.profile, &self.hub)
      == (&other.project, &other.entry, other.id, &other.profile, &other.hub)
  }
}
impl DevtoolsInspectable for Props {}

/// The panel of the profile `props.entry`, keyed by it: a choice never carries over to another profile.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<ProfilePanel>(
    &format!("local-hub-profile-{}", super::row::token(props.entry.what)),
    props,
  )
}

struct ProfilePanel {
  /// The connection the select holds, by id; empty while none is.
  chosen: Signal<String>,
  /// What the panel holds: `None` while it is what is stored.
  held: Signal<Holding>,
  /// The choices on their way to what the panel holds.
  choices: Choices,
  /// Publishes what the panel holds.
  save: edit::Save,
  /// What a save would widen, which waits for the person.
  pending: edit::Pending,
  /// The connection the panel shows, which a choice in the select differs from.
  shown_connection: Ref<Option<ConnectionId>>,
  /// The profile as the hub lists it now, and when the last save ended ([`Followed`]).
  listed: (Ref<Option<LocalProfile>>, Ref<Option<LocalProfile>>),
  unsaved: UnsavedSignal,
  what: Archivable,
  /// The role as typed ([`role`]).
  typed: role::Typed,
  /// Whether the input that pressed *Save* may decide what the save changes in the role's prompts.
  decided: Ref<bool>,
  gate: Option<InputGate>,
}

impl Component for ProfilePanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let project = props.project.open.project.id();
    let held = ctx.signal(Holding::default());
    let save = edit::save(ctx, root.clone(), props.id);
    let pending: edit::Pending = ctx.signal(None);
    let listed = (ctx.create_ref(None), ctx.create_ref(None));
    let followed = Followed {
      pending: pending.clone(),
      held: held.clone(),
      now: listed.0.clone(),
      when: listed.1.clone(),
    };
    edit::follow(ctx, &save, &followed, project);
    let decided = ctx.create_ref(false);
    let saving = (save.clone(), pending.clone(), held.clone(), decided.clone());
    let choices = Choices::new(ctx, (root, props.id), &held, move || {
      if let Some(body) = saving.2.get_untracked().body() {
        publish(&saving.0, &saving.1, body, saving.3.get());
      }
    });
    let current = props.profile.current;
    let this = Self {
      chosen: ctx.signal(current.map_or_else(String::new, |connection| connection.to_string())),
      held,
      choices,
      save,
      pending,
      shown_connection: ctx.create_ref(current),
      listed,
      unsaved: props.unsaved,
      what: props.entry.what,
      typed: ctx.signal(None),
      decided,
      gate: ctx.use_context::<InputGate>(),
    };
    this.follow_select(ctx);
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      project,
      entry,
      id,
      profile: listed,
      hub,
      ..
    } = ctx.props::<Props>().clone();
    let holding = self.holding(&listed);
    let shown = holding.0.as_ref().map_or(&listed, |held| &held.shown);
    let choosing = self.choices.checking();
    self.keep_select(shown, choosing);
    // A choice under way is a change already: *Save* and *Cancel* take it at once, *Save* once it is held.
    let role_refused = role::refused(&self.typed);
    let changed = choosing || role_refused.is_some() || holding.0.as_ref().is_some_and(|held| held.changed);
    let refused = self.refusal();
    mark_unsaved(
      &self.unsaved,
      self.what,
      (changed, refused.is_some() || role_refused.is_some()),
    );
    let parts = ready::parts(ctx, &hub, shown);
    let missing = ready::missing(&parts);
    let busy = self.busy();
    let locked = busy != Busy::Idle;
    let navigation = Navigation::of(ctx);
    let press = |fix: &Fix| self.press(fix, (&project, shown), &navigation);
    let head_end = head_end(ctx, &project, &entry, (id, PREFIX));
    let widening = self
      .pending
      .get()
      .map(|asked| edit::question(ctx, &asked, (&self.save, &self.pending)));
    let rows = table::Rows {
      profile: shown,
      chosen: &self.chosen,
      on_edit: self.on_edit(),
      choosing,
      busy: locked,
      add_connection: navigation.to(Route::Accounts(AccountsView::default())),
    };
    let body = body()
      .child(head_named(
        &entry.name,
        (
          ctx.t(kind_where_key(entry.kind)).to_string(),
          Some(words::version_line(ctx, shown, entry.number)),
        ),
        Some(head_end),
      ))
      .child(chips_of(
        words::chips(ctx, &entry, shown, &parts),
        words::used_by(ctx, shown),
      ))
      .with_children(widening)
      .child(role::section(ctx, &self.typed, shown, (self.on_edit(), locked)))
      .child(table::section(ctx, &rows))
      .child(section(
        ctx,
        (&ctx.t("local_hub.readiness.title"), &parts),
        missing,
        (&press, busy),
      ))
      .with_children(
        refused.map(|error| -> Element { field_error("local-hub-profile-error", &message_text(ctx, &error)).into() }),
      );
    let bar = save_bar::bar(
      ctx,
      PREFIX,
      (
        self.save_now(ctx, (changed, role_refused.as_ref())),
        (changed && busy == Busy::Idle).then(|| self.cancel_now()),
      ),
      save_bar::makes(ctx, entry.number),
    );
    frame(entry.what, body, Some(bar))
  }
}

impl Drop for ProfilePanel {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, self.what);
  }
}

impl ProfilePanel {
  /// What the panel holds. After a save it shows what it held, as nothing changed, until the hub lists the profile it
  /// published; then what the hub lists.
  fn holding(&self, listed: &LocalProfile) -> Holding {
    let (now, when) = &self.listed;
    now.set(Some(listed.clone()));
    let holding = self.held.get();
    let saved = holding.0.as_ref().is_some_and(|held| !held.changed);
    if saved && when.get().is_some_and(|then| then != *listed) {
      when.set(None);
      self.held.set(Holding::default());
      if self.typed.get_untracked().is_some() {
        self.typed.set(None);
      }
      return Holding::default();
    }
    holding
  }

  /// A choice in the connection's select is a change of what the panel holds, unless it is the one it shows.
  fn follow_select(&self, ctx: &mut Ctx) {
    let (shown, choices, pending) = (
      self.shown_connection.clone(),
      self.choices.clone(),
      self.pending.clone(),
    );
    ctx.watch(&self.chosen, move |value| {
      let Ok(connection) = value.parse::<ConnectionId>() else {
        return;
      };
      if shown.get() != Some(connection) {
        clear(&pending);
        choices.make(Edit::Connection(connection));
      }
    });
  }

  /// The select shows the connection the panel shows, once no choice is being checked.
  fn keep_select(&self, shown: &LocalProfile, choosing: bool) {
    self.shown_connection.set(shown.current);
    let wanted = shown
      .current
      .map_or_else(String::new, |connection| connection.to_string());
    if !choosing && self.chosen.get_untracked() != wanted {
      self.chosen.set(wanted);
    }
  }

  /// A save is under way: nothing is changed meanwhile. A choice being checked locks nothing.
  fn busy(&self) -> Busy {
    if self.save.state().get().is_pending() {
      Busy::Taking
    } else {
      Busy::Idle
    }
  }

  /// What changing a field does: makes the change on what the panel holds, dropping a question about a save.
  fn on_edit(&self) -> table::OnEdit {
    let (choices, pending) = (self.choices.clone(), self.pending.clone());
    Arc::new(move |change| {
      clear(&pending);
      choices.make(change);
    })
  }

  /// What *Save* does now: nothing while nothing differs, why it waits while the role as typed cannot be saved, else
  /// it publishes what the panel holds, once the choices being checked are held. What the save changes in the role's
  /// prompts is decided by the input that pressed it.
  fn save_now(&self, ctx: &Ctx, (changed, role): (bool, Option<&RoleErrors>)) -> Save {
    if self.save.state().get().is_pending() {
      return Save::Saving;
    }
    if !changed {
      return Save::Unchanged;
    }
    if let Some(why) = role.and_then(RoleErrors::first) {
      return Save::Waits(message_text(ctx, why));
    }
    let (choices, save, pending, held) = (
      self.choices.clone(),
      self.save.clone(),
      self.pending.clone(),
      self.held.clone(),
    );
    let (decided, gate) = (self.decided.clone(), self.gate.clone());
    Save::Ready(Press::run(move || {
      decided.set(input_may_decide(gate.as_ref(), Guarded::PermissionWidening));
      if choices.waits() {
        choices.save_after();
      } else if let Some(body) = held.get_untracked().body() {
        publish(&save, &pending, body, decided.get());
      }
    }))
  }

  /// *Cancel*: the panel shows the published version again at once, the role as typed and the choices being checked
  /// dropped. A draft stored meanwhile is left as it is: the panel never showed it.
  fn cancel_now(&self) -> Press {
    let (held, pending, choices) = (self.held.clone(), self.pending.clone(), self.choices.clone());
    let typed = self.typed.clone();
    Press::run(move || {
      choices.drop_all();
      held.set(Holding::default());
      typed.set(None);
      clear(&pending);
    })
  }

  /// What a press on `fix` does: a connection to use is a change of what the panel holds; the others open where the
  /// missing part is set.
  fn press(&self, fix: &Fix, (project, shown): (&ShownProject, &LocalProfile), navigation: &Navigation) -> Press {
    match fix {
      Fix::UseConnection => {
        let target = self
          .chosen
          .get_untracked()
          .parse::<ConnectionId>()
          .ok()
          .or_else(|| shown.connections.first().map(|choice| choice.id));
        let choices = self.choices.clone();
        Press::run(move || {
          if let Some(connection) = target {
            choices.make(Edit::Connection(connection));
          }
        })
      }
      Fix::AddConnection | Fix::SignIn => Press::Open(navigation.to(Route::Accounts(AccountsView::default()))),
      Fix::SetPath(server) | Fix::AddValue(server) => Press::Open(navigation.to(Route::Project(
        project.open.project.id(),
        ProjectPage::hub(view_of(HubFilter::McpServers, Some(Archivable::McpServer(*server)))),
      ))),
    }
  }

  /// Why the last choice or save did nothing, as the workspace said.
  fn refusal(&self) -> Option<Message> {
    let save = self.save.state().get();
    self
      .choices
      .refusal()
      .or_else(|| save.error.clone().filter(|_| save.is_rejected()))
  }
}

/// Publishes `held`, what the panel holds, dropping a question a save asked before; `decided` as [`Ask::decided`].
fn publish(save: &edit::Save, pending: &edit::Pending, held: AgentProfile, decided: bool) {
  clear(pending);
  save.run(Ask {
    held,
    allowed: Vec::new(),
    decided,
  });
}

/// Drops the question a save asked, while one does: a signal set to what it holds would render again for nothing.
fn clear(pending: &edit::Pending) {
  if pending.get_untracked().is_some() {
    pending.set(None);
  }
}
