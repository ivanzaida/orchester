//! The editor of a local skill's panel: the skill's name, description and SKILL.md over the skill as it is stored, and
//! the pinned *Save* and *Cancel*. Typing changes only the fields. *Save* publishes what they hold as the next version
//! at once ([`editor::save_and_publish_in`]: the name at once, as every save of a skill does), under the person's
//! consent when an MCP client presses it; *Cancel* puts the stored skill back in the fields, and throws away a draft
//! stored before. A refusal says why in place and keeps what was typed.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::Row,
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::Skill;
use orchester_store::StoreError;
use orchester_workspace::{
  activity::skill_operations,
  agents::data::VersionState,
  background,
  local_archive::Archivable,
  messages::{self, Message},
  removal,
  skills::{
    data::{ProjectSkills, SkillEntry},
    editor::{self, EditorData, SkillForm, publish_reason},
  },
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::{BODY_AREA, DESCRIPTION_AREA, PREFIX, Props, UnsavedSignal};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::{
    local_hub::{
      leave::{clear_unsaved, mark_unsaved},
      model::Entry,
      panel::{chips_of, frame, head_end, readiness, standing_chips, used_by},
      save_bar::{self, Save},
      words::kind_where_key,
    },
    team_hub::{
      panel::{body, head_named},
      rows::{self, Sub},
    },
  },
  services::Services,
  ui::{
    field::{FieldInput, field_error},
    message::message_text,
    pressable::Press,
  },
};

/// A text area's row: its id after the prefix, the keys of its label, sub and placeholder, and its least height.
struct Area {
  part: &'static str,
  label: &'static str,
  sub: &'static str,
  placeholder: &'static str,
  height: f32,
}

const DESCRIPTION: Area = Area {
  part: "description",
  label: "local_hub.skill.description",
  sub: "local_hub.skill.descriptionSub",
  placeholder: "local_hub.skill.descriptionPlaceholder",
  height: DESCRIPTION_AREA,
};
const BODY: Area = Area {
  part: "body",
  label: "local_hub.skill.body",
  sub: "local_hub.skill.bodySub",
  placeholder: "local_hub.skill.bodyPlaceholder",
  height: BODY_AREA,
};

#[derive(Clone)]
pub(super) struct EditorProps {
  pub props: Props,
  pub data: EditorData,
  /// The skill as it was read when the panel opened: its draft's content when it has one, which the fields start from.
  /// The editor keeps what it stores itself from then on.
  pub entry: SkillEntry,
  /// Its latest published version, what the fields are compared with.
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
  name: Signal<String>,
  description: Signal<String>,
  body: Signal<String>,
  stored: Signal<Stored>,
  save: FutureAction<Saving, Stored, Message>,
  /// Throws away the draft stored before, and reads what is stored then.
  cancel: FutureAction<SkillEntry, Stored, Message>,
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
    let stored = ctx.signal(Stored {
      entry: entry.clone(),
      published,
    });
    let save = save(ctx, &props.project.open);
    let cancel = cancel(ctx, &props.project.open);
    for done in [save.state(), cancel.state()] {
      let stored = stored.clone();
      ctx.watch(&done, move |state| {
        if let Some(now) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
          stored.set(now.clone());
        }
      });
    }
    Self {
      name: ctx.signal(entry.name.to_string()),
      description: ctx.signal(entry.body.description().to_owned()),
      body: ctx.signal(entry.body.body().to_owned()),
      stored,
      save,
      cancel,
      gate: ctx.use_context::<InputGate>(),
      unsaved: props.unsaved.clone(),
      what: props.entry.what,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, data, .. } = ctx.props::<EditorProps>().clone();
    let stored = self.stored.get();
    let name = self.name.get();
    let parsed = SkillForm {
      name: &name,
      description: &self.description.get(),
      body: &self.body.get(),
      files: stored.entry.body.files().to_vec(),
      scope: stored.entry.body.scope().clone(),
    }
    .skill(&data, Some(stored.entry.id));
    let changed = stored.entry.state.has_draft()
      || parsed.as_ref().map_or(true, |body| {
        *body != stored.published || *stored.entry.name != *name.trim()
      });
    let (saving, cancelling) = (self.save.state().get(), self.cancel.state().get());
    let refused = saving
      .error
      .clone()
      .filter(|_| saving.is_rejected())
      .or_else(|| cancelling.error.clone().filter(|_| cancelling.is_rejected()));
    mark_unsaved(
      &self.unsaved,
      self.what,
      (changed, refused.is_some() || parsed.is_err()),
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
    let body = body()
      .child(head)
      .child(chips_of(standing_chips(ctx, entry), None))
      .child(rows::section(
        PREFIX,
        &ctx.t("local_hub.skill.title"),
        Vec::new(),
        self.rows(ctx, (&stored.entry, entry), parsed.as_ref().err()),
      ))
      .with_children(
        refused
          .map(|error| -> Element { field_error(&format!("{PREFIX}-refused"), &message_text(ctx, &error)).into() }),
      )
      .with_children(readiness(ctx, entry));
    let busy = saving.is_pending() || cancelling.is_pending();
    let save = self.save_now(ctx, (&stored, &name, &parsed), (changed, busy));
    let bar = save_bar::bar(
      ctx,
      PREFIX,
      (save, (changed && !busy).then(|| self.cancel_now(&stored))),
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
  /// *Name*, *Used by* (the profiles that give it to their agents: shown, not chosen), *Description*, *SKILL.md* and
  /// *Files*.
  fn rows(&self, ctx: &mut Ctx, (stored, entry): (&SkillEntry, &Entry), error: Option<&Message>) -> Vec<Row> {
    let mut name = FieldInput::new(
      format!("{PREFIX}-name-input"),
      ctx.t("local_hub.skill.name"),
      self.name.clone(),
    )
    .width(Dimension::Pct(100.0))
    .mono();
    let name_sub = match error {
      Some(error) => {
        name = name.invalid();
        Sub {
          words: message_text(ctx, error),
          color: PaletteColor::Danger,
        }
      }
      None => Sub::quiet(ctx.t("local_hub.skill.nameSub")),
    };
    let files = std::iter::once("SKILL.md".to_owned())
      .chain(stored.body.files().iter().map(|file| file.path.as_str().to_owned()));
    vec![
      rows::row(
        &format!("{PREFIX}-name"),
        &ctx.t("local_hub.skill.name"),
        Some(name_sub),
        name.render(ctx),
      ),
      used_by(ctx, PREFIX, entry),
      area_row(ctx, &DESCRIPTION, &self.description),
      area_row(ctx, &BODY, &self.body),
      rows::row(
        &format!("{PREFIX}-files"),
        &ctx.t("local_hub.skill.files"),
        Some(Sub::quiet(ctx.t("local_hub.skill.filesSub"))),
        rows::tags(&format!("{PREFIX}-files-value"), files),
      ),
    ]
  }

  /// What *Save* does now: nothing while nothing differs, why it waits while the fields cannot be saved or published
  /// (a name that is taken, a blank description or body), else it publishes them.
  fn save_now(
    &self,
    ctx: &Ctx,
    (stored, name, parsed): (&Stored, &str, &Result<Skill, Message>),
    (changed, busy): (bool, bool),
  ) -> Save {
    if busy {
      return Save::Saving;
    }
    if !changed {
      return Save::Unchanged;
    }
    let body = match parsed {
      Ok(body) => body.clone(),
      Err(error) => return Save::Waits(message_text(ctx, error)),
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
      if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
        save.run(saving.clone());
      }
    }))
  }

  /// *Cancel*: the fields take the stored name and the published version at once; a draft stored before is thrown
  /// away too.
  fn cancel_now(&self, stored: &Stored) -> Press {
    let fields = (self.name.clone(), self.description.clone(), self.body.clone());
    let (cancel, stored) = (self.cancel.clone(), stored.clone());
    Press::run(move || {
      fields.0.set(stored.entry.name.to_string());
      fields.1.set(stored.published.description().to_owned());
      fields.2.set(stored.published.body().to_owned());
      if stored.entry.state.has_draft() {
        cancel.run(stored.entry.clone());
      }
    })
  }
}

/// The row of the text area `area`, typing into `value`; its label stays at the top.
fn area_row(ctx: &mut Ctx, area: &Area, value: &Signal<String>) -> Row {
  let label = ctx.t(area.label);
  let input = FieldInput::new(format!("{PREFIX}-{}-input", area.part), label.clone(), value.clone())
    .placeholder(ctx.t(area.placeholder))
    .width(Dimension::Pct(100.0))
    .text_area()
    .area_height(area.height);
  rows::row(
    &format!("{PREFIX}-{}", area.part),
    &label,
    Some(Sub::quiet(ctx.t(area.sub))),
    input.render(ctx),
  )
  .align_items(Alignment::Start)
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

/// Throws away the draft `entry` holds, while it still is the stored one, and reads the skill then.
fn cancel(ctx: &mut Ctx, project: &OpenProject) -> FutureAction<SkillEntry, Stored, Message> {
  let (project, services) = (project.clone(), Services::of(ctx));
  ctx.future_action(move |entry: SkillEntry| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      let root = project.repo_root.clone();
      let stored = background::run(move || {
        removal::discard_in::<Skill>(&root, entry.id, &entry.body)?;
        ProjectSkills::load_in(&root)
          .and_then(|listed| {
            listed
              .entry(entry.id)
              .cloned()
              .ok_or(StoreError::ResourceNotFound(entry.id))
          })
          .map_err(|error| messages::store_error(&error))
      })
      .await??;
      services.changes.record(project.project.id(), Outside::Skills);
      Ok(Stored::published(stored))
    }
  })
}
