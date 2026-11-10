//! The editor of a local command tool's panel: the form that makes one ([`ToolFields`]) over its published version, and
//! the pinned *Save* and *Cancel*. Typing changes only the fields. *Save* publishes what they hold as the next version
//! at once ([`local_edit::save_and_publish_in`]), over a draft stored meanwhile too; what it widens beyond what the
//! person allowed is asked first, in the panel, and only a person's press allows it. *Cancel* puts the published
//! version back in the fields. A refusal says why in place and keeps what was typed.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ResourceId,
  resource::{Tool, VersionNumber},
};
use orchester_workspace::{
  activity::operations,
  background,
  local_archive::Archivable,
  messages::Message,
  tasks::outside::Outside,
  tools::{
    data::ToolChange,
    local_edit::{self, Saved},
  },
  workspace::OpenProject,
};

use super::{PREFIX, Props, UnsavedSignal};
use crate::{
  mcp::input_gate::Guarded,
  screens::{
    connections::widen::{NoticeProps, WidenLook, WidenNotice, WidenProps},
    hub::forms::{Form, Ids, ToolFields},
    local_hub::{
      leave::{clear_unsaved, mark_unsaved},
      model::Entry,
      panel::{chips_of, frame, head_end, readiness, standing_chips, used_by},
      save_bar::{self, Save},
      words::kind_where_key,
    },
    team_hub::{
      panel::{body, head_named},
      rows,
    },
  },
  services::Services,
  ui::{field::field_error, icon::Icon, message::message_text, pressable::Press},
};

/// The ids of the form's parts, which tests and automation address: `local-hub-tool-program-input`,
/// `local-hub-tool-arguments-input`, `local-hub-tool-timeout-input`, …
const IDS: Ids = Ids {
  rows: PREFIX,
  sections: PREFIX,
};

#[derive(Clone)]
pub(super) struct EditorProps {
  pub props: Props,
  /// Its latest published version, which the fields start from and are compared with; its draft while it has only one.
  pub published: Tool,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.props == other.props
  }
}
impl DevtoolsInspectable for EditorProps {}

/// A save: the version the panel started from, what it publishes, and what the person allowed it to widen.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Ask {
  base: Tool,
  tool: Tool,
  allowed: Vec<Message>,
}

/// A save that ended, and what the workspace said of it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Done {
  ask: Ask,
  saved: Saved,
}
impl DevtoolsInspectable for Done {}

/// A save that waits for the person: what it widens beyond what they allowed, and the save with all of it allowed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Asked {
  ask: Ask,
  reasons: Vec<Message>,
}
impl DevtoolsInspectable for Asked {}

/// The published version the fields are compared with, and a save starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Published(Tool);
impl DevtoolsInspectable for Published {}

pub(super) struct Editor {
  fields: ToolFields,
  stored: Signal<Published>,
  save: FutureAction<Ask, Done, Message>,
  /// What a save would widen, which waits for the person.
  pending: Signal<Option<Asked>>,
  unsaved: UnsavedSignal,
  what: Archivable,
}

impl Component for Editor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps { props, published } = ctx.props::<EditorProps>().clone();
    let save = save(ctx, &props.project.open, &props.entry, props.id);
    let stored = ctx.signal(Published(published.clone()));
    let pending = ctx.signal(None);
    follow(ctx, &save, (&stored, &pending));
    let fields = ToolFields::new(ctx);
    fields.load(&published);
    Self {
      fields,
      stored,
      save,
      pending,
      unsaved: props.unsaved.clone(),
      what: props.entry.what,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, .. } = ctx.props::<EditorProps>().clone();
    let Published(stored) = self.stored.get();
    let saving = self.save.state().get();
    let busy = saving.is_pending();
    let drawn = self.fields.draw(
      ctx,
      &Form {
        ids: IDS,
        base: Some(&stored),
        name: None,
        tried: true,
        locked: busy,
      },
    );
    let changed = drawn.made.as_ref().map_or(true, |tool| *tool != stored);
    let refused = saving.error.clone().filter(|_| saving.is_rejected());
    mark_unsaved(
      &self.unsaved,
      self.what,
      (changed, refused.is_some() || drawn.made.is_err()),
    );
    let entry = &props.entry;
    let head = head_named(
      &entry.name,
      (
        ctx.t(kind_where_key(entry.kind)).to_string(),
        entry.number.map(|number| number.to_string()),
      ),
      Some(head_end(ctx, &props.project, entry, (props.id, PREFIX))),
    );
    let widening = self.pending.get().map(|asked| self.question(ctx, &asked));
    let rows = [used_by(ctx, PREFIX, entry)].into_iter().chain(drawn.rows).collect();
    let body = body()
      .child(head)
      .child(chips_of(standing_chips(ctx, entry), None))
      .with_children(widening)
      .child(rows::section(PREFIX, &ctx.t("local_hub.tool.title"), Vec::new(), rows))
      .with_children(
        refused
          .map(|error| -> Element { field_error(&format!("{PREFIX}-refused"), &message_text(ctx, &error)).into() }),
      )
      .with_children(readiness(ctx, entry));
    let save = self.save_now((&stored, drawn.made), (changed, busy));
    let bar = save_bar::bar(
      ctx,
      PREFIX,
      (save, (changed && !busy).then(|| self.cancel_now(&stored))),
      save_bar::makes(ctx, entry.number),
    );
    frame(entry.what, body, Some(bar))
  }
}

impl Drop for Editor {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, self.what);
  }
}

impl Editor {
  /// What *Save* does now: nothing while nothing differs, why it waits while the fields make no tool, else it publishes
  /// them, asking nothing allowed yet.
  fn save_now(&self, (stored, made): (&Tool, Result<Tool, Arc<str>>), (changed, busy): (bool, bool)) -> Save {
    if busy {
      return Save::Saving;
    }
    if !changed {
      return Save::Unchanged;
    }
    let tool = match made {
      Ok(tool) => tool,
      Err(why) => return Save::Waits(why),
    };
    let (save, pending) = (self.save.clone(), self.pending.clone());
    let ask = Ask {
      base: stored.clone(),
      tool,
      allowed: Vec::new(),
    };
    Save::Ready(Press::run(move || {
      clear(&pending);
      save.run(ask.clone());
    }))
  }

  /// *Cancel*: the fields take the published version at once, and a question a save asked goes. A draft stored
  /// meanwhile is left as it is: the panel never showed it.
  fn cancel_now(&self, stored: &Tool) -> Press {
    let (fields, stored, pending) = (self.fields.clone(), stored.clone(), self.pending.clone());
    Press::run(move || {
      fields.load(&stored);
      clear(&pending);
    })
  }

  /// The question of a save that widens, in the panel: what it widens, *Allow* and *Keep editing*. Only a person
  /// answers it.
  fn question(&self, ctx: &mut Ctx, asked: &Asked) -> Element {
    let mut reasons: Vec<Arc<str>> = asked.reasons.iter().map(|reason| message_text(ctx, reason)).collect();
    reasons.push(ctx.t("local_hub.tool.execution"));
    let (keep, pending, save, ask) = (
      self.pending.clone(),
      self.pending.clone(),
      self.save.clone(),
      asked.ask.clone(),
    );
    let question = WidenProps {
      id: "local-hub-tool-confirm".into(),
      look: WidenLook::Band,
      affirming: false,
      automation: Guarded::PermissionWidening,
      title: ctx.t("local_hub.tool.widenTitle"),
      reasons: reasons.into(),
      confirm: ctx.t("local_hub.edit.allow"),
      keep: ctx.t("local_hub.edit.keep"),
      on_keep: Arc::new(move || keep.set(None)),
      on_confirm: Arc::new(move || {
        pending.set(None);
        save.run(ask.clone());
      }),
    };
    ctx.mount_keyed::<WidenNotice>(
      "local-hub-tool-confirm",
      NoticeProps {
        question,
        confirm_icon: Icon::Check,
      },
    )
  }
}

/// Drops the question a save asked, while one does: a signal set to what it holds would render again for nothing.
fn clear(pending: &Signal<Option<Asked>>) {
  if pending.get_untracked().is_some() {
    pending.set(None);
  }
}

/// What follows a save that ended: the fields are compared with what it published from then on; a widening is asked.
fn follow(
  ctx: &mut Ctx,
  save: &FutureAction<Ask, Done, Message>,
  (stored, pending): (&Signal<Published>, &Signal<Option<Asked>>),
) {
  let (stored, pending) = (stored.clone(), pending.clone());
  ctx.watch(&save.state(), move |state| {
    let Some(done) = state.data.clone().filter(|_| state.is_fulfilled()) else {
      return;
    };
    match done.saved {
      Saved::Published(tool) => stored.set(Published(tool)),
      Saved::Confirm(reasons) => {
        let asked = reasons
          .iter()
          .filter(|reason| !done.ask.allowed.contains(reason))
          .cloned()
          .collect();
        pending.set(Some(Asked {
          ask: Ask {
            allowed: reasons,
            ..done.ask
          },
          reasons: asked,
        }));
      }
    }
  });
}

/// Publishes what a save takes, recorded in the session's activity; the screens that show the tools read them again.
fn save(ctx: &mut Ctx, project: &OpenProject, entry: &Entry, id: ResourceId) -> FutureAction<Ask, Done, Message> {
  let (project, services, name) = (project.clone(), Services::of(ctx), entry.name.clone());
  // What the activity says it publishes; the workspace publishes the version after the latest.
  let version = entry
    .number
    .map_or(Some(VersionNumber::FIRST), VersionNumber::next)
    .map_or(0, VersionNumber::get);
  ctx.future_action(move |ask: Ask| {
    let (project, services, name) = (project.clone(), services.clone(), name.clone());
    async move {
      let spec = operations::tool_change(&project, &name, &ToolChange::Publish(id), version);
      let root = project.repo_root.clone();
      let asked = ask.clone();
      let saved = services
        .activity
        .track(spec, async move {
          background::run(move || {
            local_edit::save_and_publish_in(&root, id, (&asked.base, &asked.tool), &asked.allowed)
              .map_err(|error| error.message())
          })
          .await?
        })
        .await?;
      if matches!(saved, Saved::Published(_)) {
        services.changes.record(project.project.id(), Outside::Tools);
      }
      Ok(Done { ask, saved })
    }
  })
}
