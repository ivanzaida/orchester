//! The editor of a local skill's panel: the form that makes one ([`SkillFields`]: its name, description, files and
//! SKILL.md) over its published version, and the pinned *Save* and *Cancel*. Typing changes only the fields. *Save*
//! publishes what they hold as the next version at once ([`editor::save_and_publish_in`]: the name at once, as every
//! save of a skill does), over a draft stored meanwhile too, under the person's consent when an MCP client presses it;
//! *Cancel* puts the stored name and the published version back in the fields. A refusal says why in place and keeps
//! what was typed.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::resource::Skill;
use orchester_workspace::{
  activity::skill_operations,
  agents::data::VersionState,
  background,
  local_archive::Archivable,
  messages::Message,
  skills::{
    data::SkillEntry,
    editor::{self, EditorData, publish_reason},
  },
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::{PREFIX, Props, UnsavedSignal};
use crate::{
  mcp::input_gate::{Guarded, InputGate, may_press},
  screens::{
    hub::forms::{Form, Ids, Naming, SkillFields},
    local_hub::{
      leave::{clear_unsaved, mark_unsaved},
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
  ui::{field::field_error, message::message_text, pressable::Press},
};

/// The ids of the form's parts, which tests and automation address: `local-hub-skill-name-input`,
/// `local-hub-skill-description-input`, `local-hub-skill-skill-md-input`, …
const IDS: Ids = Ids {
  rows: PREFIX,
  sections: PREFIX,
};

#[derive(Clone)]
pub(super) struct EditorProps {
  pub props: Props,
  pub data: EditorData,
  /// The skill as it was read when the panel opened, with a draft stored before if it has one: what a save is checked
  /// against. The editor keeps what it stores itself from then on.
  pub entry: SkillEntry,
  /// Its latest published version, which the fields start from and are compared with; its draft while it has only one.
  pub published: Skill,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.props == other.props && self.data == other.data
  }
}
impl DevtoolsInspectable for EditorProps {}

/// The skill as it is stored: the entry (with a draft stored before, if any) and its published version.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
  entry: SkillEntry,
  published: Skill,
}
impl DevtoolsInspectable for Stored {}

impl Stored {
  /// What a write that left no draft stored: `entry` is its published version.
  fn published(entry: SkillEntry) -> Self {
    let published = entry.body.clone();
    Self { entry, published }
  }
}

/// What a save takes: the entry the panel showed, the name and the body.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Saving {
  entry: SkillEntry,
  name: String,
  body: Skill,
}
impl DevtoolsInspectable for Saving {}

pub(super) struct Editor {
  fields: SkillFields,
  stored: Signal<Stored>,
  save: FutureAction<Saving, Stored, Message>,
  /// Publishing reaches agents: an MCP client's input publishes only under the person's consent.
  gate: Option<InputGate>,
  unsaved: UnsavedSignal,
  what: Archivable,
}

impl Component for Editor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let EditorProps {
      props,
      entry,
      published,
      ..
    } = ctx.props::<EditorProps>().clone();
    let save = save(ctx, &props.project.open);
    let saved = ctx.signal(Stored {
      entry: entry.clone(),
      published: published.clone(),
    });
    let stored = saved.clone();
    ctx.watch(&save.state(), move |state| {
      if let Some(now) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        saved.set(now.clone());
      }
    });
    let fields = SkillFields::new(ctx);
    fields.load_name(&entry.name);
    fields.load(&published);
    Self {
      fields,
      stored,
      save,
      gate: ctx.use_context::<InputGate>(),
      unsaved: props.unsaved.clone(),
      what: props.entry.what,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, data, .. } = ctx.props::<EditorProps>().clone();
    let stored = self.stored.get();
    let name = self.fields.name();
    let saving = self.save.state().get();
    let busy = saving.is_pending();
    let named = data.validate_name(Some(stored.entry.id), &name);
    let drawn = self.fields.draw(
      ctx,
      &Form {
        ids: IDS,
        base: Some(&stored.published),
        name: Some(Naming {
          said: named.as_ref().err().map(|error| message_text(ctx, error)),
          first: None,
        }),
        tried: true,
        locked: busy,
      },
    );
    let made = match named {
      Ok(_) => drawn.made,
      Err(error) => Err(message_text(ctx, &error)),
    };
    let differs = made.as_ref().map_or(true, |body| {
      *body != stored.published || *stored.entry.name != *name.trim()
    });
    // A skill that has only a draft so far has it to publish: what it holds is not out yet.
    let changed = differs || stored.entry.state.published().is_none();
    let refused = saving.error.clone().filter(|_| saving.is_rejected());
    mark_unsaved(&self.unsaved, self.what, (changed, refused.is_some() || made.is_err()));
    let entry = &props.entry;
    let head = head_named(
      &entry.name,
      (
        ctx.t(kind_where_key(entry.kind)).to_string(),
        entry.number.map(|number| number.to_string()),
      ),
      Some(head_end(ctx, &props.project, entry, (props.id, PREFIX))),
    );
    let rows = drawn
      .name
      .into_iter()
      .chain([used_by(ctx, PREFIX, entry)])
      .chain(drawn.rows)
      .collect();
    let body = body()
      .child(head)
      .child(chips_of(standing_chips(ctx, entry), None))
      .child(rows::section(PREFIX, &ctx.t("local_hub.skill.title"), Vec::new(), rows))
      .with_children(drawn.sections)
      .with_children(
        refused
          .map(|error| -> Element { field_error(&format!("{PREFIX}-refused"), &message_text(ctx, &error)).into() }),
      )
      .with_children(readiness(ctx, entry));
    let save = self.save_now(ctx, (&stored, &name, made), (changed, busy));
    let bar = save_bar::bar(
      ctx,
      PREFIX,
      (save, (differs && !busy).then(|| self.cancel_now(&stored))),
      save_bar::makes(ctx, stored.entry.state.published()),
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
  /// What *Save* does now: nothing while nothing differs, why it waits while the fields cannot be saved or published
  /// (a name that is taken, a blank description or body), else it publishes them.
  fn save_now(
    &self,
    ctx: &Ctx,
    (stored, name, made): (&Stored, &str, Result<Skill, Arc<str>>),
    (changed, busy): (bool, bool),
  ) -> Save {
    if busy {
      return Save::Saving;
    }
    if !changed {
      return Save::Unchanged;
    }
    let body = match made {
      Ok(body) => body,
      Err(why) => return Save::Waits(why),
    };
    // Asked of the body as a draft to publish: what the Skills editor refuses to publish.
    let candidate = SkillEntry {
      body: body.clone(),
      state: VersionState::Draft {
        published: stored.entry.state.published(),
      },
      ..stored.entry.clone()
    };
    if let Some(reason) = publish_reason(&candidate) {
      return Save::Waits(message_text(ctx, &reason));
    }
    let (save, gate) = (self.save.clone(), self.gate.clone());
    let saving = Saving {
      entry: stored.entry.clone(),
      name: name.trim().to_owned(),
      body,
    };
    Save::Ready(Press::run(move || {
      if may_press(
        gate.as_ref(),
        Guarded::PermissionWidening,
        "publish a skill, which reaches agents' prompts",
      ) {
        save.run(saving.clone());
      }
    }))
  }

  /// *Cancel*: the fields take the stored name and the published version at once. A draft stored meanwhile is left as
  /// it is: the panel never showed it.
  fn cancel_now(&self, stored: &Stored) -> Press {
    let (fields, stored) = (self.fields.clone(), stored.clone());
    Press::run(move || {
      fields.load_name(&stored.entry.name);
      fields.load(&stored.published);
    })
  }
}

/// Publishes what a save takes, recorded in the session's activity; the screens that show the skills read them again.
fn save(ctx: &mut Ctx, project: &OpenProject) -> FutureAction<Saving, Stored, Message> {
  let (project, services) = (project.clone(), Services::of(ctx));
  ctx.future_action(move |saving: Saving| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      let next = saving
        .entry
        .state
        .published()
        .map_or(1, |version| version.get().saturating_add(1));
      let spec = skill_operations::skill_publish(&project, saving.entry.id, &saving.name, next);
      let root = project.repo_root.clone();
      let saved = services
        .activity
        .track(spec, async move {
          background::run(move || {
            editor::save_and_publish_in(&root, &saving.entry, &saving.name, &saving.body)
              .map_err(|error| error.message())
          })
          .await?
        })
        .await?;
      services.changes.record(project.project.id(), Outside::Skills);
      Ok(Stored::published(saved))
    }
  })
}
