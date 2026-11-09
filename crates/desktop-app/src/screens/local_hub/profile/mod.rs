//! The panel of a local profile (`Agents · Profile` L1 `fFxoM`, `docs/design/team-configuration.md` rule 11): the head
//! with the version and its *Versions*, the chips (how it stands, who uses it), *Profile* (CLI, connection, model,
//! effort, MCP servers, secrets), *Ready?* with the fix of what is missing, and the pinned *Save* and *Cancel*
//! ([`super::save_bar`]). The parts are the team profile's ([`crate::screens::team_hub::profile`]) over the local
//! profile store. The profile is edited in place, as the design draws it ([`table`], [`pick`], [`servers`]): the CLI,
//! the connection, the model, the reasoning effort and the MCP servers are selects, and each choice changes only what
//! the panel holds, at once and without locking anything it shows ([`choices`]). *Save* publishes it as the next
//! version at once ([`edit`]); *Cancel* puts back what is stored.
//!
//! A draft stored before (by the profile's page) opens as the panel's changes over the published version: *Save*
//! publishes it, with what the panel changed on top, and *Cancel* throws it away.

mod choices;
mod edit;
mod pick;
mod ready;
pub(in crate::screens) mod servers;
mod table;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::{Ref, Signal},
  node::Element,
};
use orchester_core::{
  id::{ConnectionId, ResourceId},
  time::Timestamp,
};
use orchester_workspace::{
  agents::{
    local_edit::{self, Edit},
    local_hub::{LocalHub, LocalProfile},
  },
  background,
  local_archive::Archivable,
  messages::Message,
  tasks::outside::Outside,
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
  navigation::Navigation,
  project::ShownProject,
  routes::{AccountsView, HubFilter, ProjectPage, Route},
  screens::team_hub::{
    actions::Busy,
    panel::{body, head_named},
    profile::ready::section,
  },
  services::Services,
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
  /// Throws away the draft stored before, the one saved at the time it takes.
  cancel: FutureAction<Timestamp, (), Message>,
  /// What a save would widen, which waits for the person.
  pending: edit::Pending,
  /// The connection the panel shows, which a choice in the select differs from.
  shown_connection: Ref<Option<ConnectionId>>,
  /// The profile as the hub lists it now, and when the last save ended ([`Followed`]).
  listed: (Ref<Option<LocalProfile>>, Ref<Option<LocalProfile>>),
  unsaved: UnsavedSignal,
  what: Archivable,
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
    let saving = (save.clone(), pending.clone(), held.clone());
    let choices = Choices::new(ctx, (root.clone(), props.id), &held, move || {
      publish(&saving.0, &saving.1, saving.2.get_untracked().body());
    });
    let cancel = cancel(ctx, (root, project, props.id));
    let current = props.profile.current;
    let this = Self {
      chosen: ctx.signal(current.map_or_else(String::new, |connection| connection.to_string())),
      held,
      choices,
      save,
      cancel,
      pending,
      shown_connection: ctx.create_ref(current),
      listed,
      unsaved: props.unsaved,
      what: props.entry.what,
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
    let changed = choosing || listed.draft || holding.0.as_ref().is_some_and(|held| held.changed);
    let refused = self.refusal();
    mark_unsaved(&self.unsaved, self.what, (changed, refused.is_some()));
    let parts = ready::parts(ctx, &hub, shown);
    let missing = ready::missing(&parts);
    let busy = self.busy();
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
      busy: busy != Busy::Idle,
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
        self.save_now(changed),
        (changed && busy == Busy::Idle).then(|| self.cancel_now(&listed)),
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

  /// A save or a cancel is under way: nothing is changed meanwhile. A choice being checked locks nothing.
  fn busy(&self) -> Busy {
    if self.save.state().get().is_pending() || self.cancel.state().get().is_pending() {
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

  /// What *Save* does now: nothing while nothing differs, else it publishes what the panel holds, once the choices
  /// being checked are held.
  fn save_now(&self, changed: bool) -> Save {
    if self.save.state().get().is_pending() {
      return Save::Saving;
    }
    if !changed {
      return Save::Unchanged;
    }
    let (choices, save, pending, held) = (
      self.choices.clone(),
      self.save.clone(),
      self.pending.clone(),
      self.held.clone(),
    );
    Save::Ready(Press::run(move || {
      if choices.waits() {
        choices.save_after();
      } else {
        publish(&save, &pending, held.get_untracked().body());
      }
    }))
  }

  /// *Cancel*: the panel shows what is stored again at once, the choices being checked dropped; a draft stored before
  /// is thrown away too.
  fn cancel_now(&self, listed: &LocalProfile) -> Press {
    let (held, pending, choices, cancel) = (
      self.held.clone(),
      self.pending.clone(),
      self.choices.clone(),
      self.cancel.clone(),
    );
    let draft = listed.draft_saved_at;
    Press::run(move || {
      choices.drop_all();
      held.set(Holding::default());
      clear(&pending);
      if let Some(saved) = draft {
        cancel.run(saved);
      }
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

  /// Why the last choice, save or cancel did nothing, as the workspace said.
  fn refusal(&self) -> Option<Message> {
    let (save, cancel) = (self.save.state().get(), self.cancel.state().get());
    self
      .choices
      .refusal()
      .or_else(|| save.error.clone().filter(|_| save.is_rejected()))
      .or_else(|| cancel.error.clone().filter(|_| cancel.is_rejected()))
  }
}

/// Publishes `held` (`None` for what is stored), dropping a question a save asked before.
fn publish(save: &edit::Save, pending: &edit::Pending, held: Option<orchester_core::resource::AgentProfile>) {
  clear(pending);
  save.run(Ask {
    held,
    allowed: Vec::new(),
  });
}

/// Drops the question a save asked, while one does: a signal set to what it holds would render again for nothing.
fn clear(pending: &edit::Pending) {
  if pending.get_untracked().is_some() {
    pending.set(None);
  }
}

/// Throws away the draft of the profile `id` saved at the time it takes, while it is still that one; the screens that
/// show the profiles read them again.
fn cancel(
  ctx: &mut Ctx,
  (root, project, id): (std::path::PathBuf, orchester_core::id::ProjectId, ResourceId),
) -> FutureAction<Timestamp, (), Message> {
  let changes = Services::of(ctx).changes;
  ctx.future_action(move |shown: Timestamp| {
    let (root, changes) = (root.clone(), changes.clone());
    async move {
      background::run(move || local_edit::discard(&root, id, shown)).await??;
      for change in [Outside::Profiles, Outside::Pipelines] {
        changes.record(project, change);
      }
      Ok(())
    }
  })
}
