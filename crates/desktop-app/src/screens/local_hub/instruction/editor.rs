//! The editor of a local instruction's panel: the Instructions editor's fields ([`Fields`]) over the instruction as it
//! is stored, and the pinned *Save* and *Cancel*. Typing changes only the fields. *Save* publishes what they hold as
//! the next version at once ([`editor::save_and_publish_in`]), under the person's consent when an MCP client presses
//! it; *Cancel* puts the published version back in the fields, and throws away a draft stored before. A refusal says
//! why in place and keeps what was typed.

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
use orchester_core::resource::Instruction;
use orchester_workspace::{
  activity::operations,
  background,
  instructions::{
    data::{InstructionEntry, entry_in},
    editor,
    form::{Events, FormErrors, ScopeChoice, SettingsError, Target},
  },
  local_archive::Archivable,
  messages::{self, Message},
  removal,
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::{PREFIX, Props, TEXT_AREA, UnsavedSignal};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  screens::{
    instructions::{fields::Fields, state::Tab},
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

#[derive(Clone)]
pub(super) struct EditorProps {
  pub props: Props,
  /// The instruction as it was read when the panel opened: its draft's content when it has one, which the fields
  /// start from. The editor keeps what it stores itself from then on.
  pub entry: InstructionEntry,
  /// Its latest published version, what the fields are compared with.
  pub published: Instruction,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.props == other.props
  }
}
impl DevtoolsInspectable for EditorProps {}

/// The instruction as it is stored: the entry (with a draft stored before, if any) and its published version.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
  entry: InstructionEntry,
  published: Instruction,
}
impl DevtoolsInspectable for Stored {}

impl Stored {
  /// What a write that left no draft stored: `entry` is its published version.
  fn published(entry: InstructionEntry) -> Self {
    let published = entry.body.clone();
    Self { entry, published }
  }
}

pub(super) struct Editor {
  fields: Fields,
  stored: Signal<Stored>,
  /// Publishes what the fields hold over the stored entry.
  save: FutureAction<(InstructionEntry, Instruction), Stored, Message>,
  /// Throws away the draft stored before, and reads what is stored then.
  cancel: FutureAction<InstructionEntry, Stored, Message>,
  /// Publishing reaches agents' prompts: an MCP client's input publishes only under the person's consent.
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
    let fields = Fields::new(ctx, Tab::of(entry.body.scope()), Some(&entry));
    let stored = ctx.signal(Stored { entry, published });
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
      fields,
      stored,
      save,
      cancel,
      gate: ctx.use_context::<InputGate>(),
      unsaved: props.unsaved,
      what: props.entry.what,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, .. } = ctx.props::<EditorProps>().clone();
    let stored = self.stored.get();
    let parsed = self.fields.instruction();
    let changed = stored.entry.state.has_draft() || parsed.as_ref().map_or(true, |body| *body != stored.published);
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
    let errors = parsed.as_ref().err();
    let body = body()
      .child(head)
      .child(chips_of(standing_chips(ctx, entry), None))
      .child(rows::section(
        PREFIX,
        &ctx.t("local_hub.instruction.title"),
        Vec::new(),
        self.rows(ctx, &props.entry, errors),
      ))
      .with_children(settings_error(ctx, errors))
      .with_children(
        refused
          .map(|error| -> Element { field_error(&format!("{PREFIX}-refused"), &message_text(ctx, &error)).into() }),
      )
      .with_children(readiness(ctx, entry));
    let busy = saving.is_pending() || cancelling.is_pending();
    let bar = save_bar::bar(
      ctx,
      PREFIX,
      (
        self.save_now(ctx, (&stored, &parsed), (changed, busy)),
        (changed && !busy).then(|| self.cancel_now(&stored)),
      ),
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
  /// *Used by*, *When* and *Text*. Who gets it is shown, not chosen: the profiles that give it to their agents.
  fn rows(&self, ctx: &mut Ctx, entry: &Entry, errors: Option<&FormErrors>) -> Vec<Row> {
    vec![
      used_by(ctx, PREFIX, entry),
      rows::row(
        &format!("{PREFIX}-when"),
        &ctx.t("local_hub.instruction.when"),
        None,
        self.fields.events_choice(ctx, true),
      ),
      self.text_row(ctx, errors.and_then(|errors| errors.text.as_ref())),
    ]
  }

  /// *Text*: the instruction's Markdown, its label at the top; a text that cannot be saved says why under the label.
  fn text_row(&self, ctx: &mut Ctx, error: Option<&Message>) -> Row {
    let label = ctx.t("local_hub.instruction.text");
    let mut input = FieldInput::new(format!("{PREFIX}-text-input"), label.clone(), self.fields.text.clone())
      .placeholder(ctx.t("local_hub.instruction.textPlaceholder"))
      .width(Dimension::Pct(100.0))
      .text_area()
      .area_height(TEXT_AREA);
    if error.is_some() {
      input = input.invalid();
    }
    let sub = match error {
      Some(error) => Sub {
        words: message_text(ctx, error),
        color: PaletteColor::Danger,
      },
      None => Sub::quiet(ctx.t("local_hub.instruction.textSub")),
    };
    rows::row(&format!("{PREFIX}-text"), &label, Some(sub), input.render(ctx)).align_items(Alignment::Start)
  }

  /// What *Save* does now: nothing while nothing differs, why it waits while the fields cannot be saved or a variable
  /// has no value where it is added, else it publishes them.
  fn save_now(
    &self,
    ctx: &Ctx,
    (stored, parsed): (&Stored, &Result<Instruction, FormErrors>),
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
      Err(errors) => return Save::Waits(waits(ctx, errors)),
    };
    if !self.fields.blocked().is_empty() {
      return Save::Waits(ctx.t("local_hub.instruction.variablesMissing"));
    }
    let (save, gate, entry) = (self.save.clone(), self.gate.clone(), stored.entry.clone());
    Save::Ready(Press::run(move || {
      if input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
        save.run((entry.clone(), body.clone()));
      }
    }))
  }

  /// *Cancel*: the fields take the published version at once; a draft stored before is thrown away too.
  fn cancel_now(&self, stored: &Stored) -> Press {
    let (fields, cancel, stored) = (self.fields.clone(), self.cancel.clone(), stored.clone());
    Press::run(move || {
      take(&fields, &stored.published);
      if stored.entry.state.has_draft() {
        cancel.run(stored.entry.clone());
      }
    })
  }
}

/// Why the fields cannot be saved, in the words of the first thing wrong.
fn waits(ctx: &Ctx, errors: &FormErrors) -> std::sync::Arc<str> {
  match (&errors.text, errors.settings) {
    (Some(text), _) => message_text(ctx, text),
    (None, Some(settings)) => message_text(ctx, &settings.message()),
    (None, None) => ctx.t("local_hub.edit.invalid"),
  }
}

/// Every field takes `body`.
fn take(fields: &Fields, body: &Instruction) {
  fields.text.set(body.text().as_str().to_owned());
  fields.targets.set(Target::of(body.targets()));
  fields.events.set(Events::of(body.events()));
  fields.scope.set(ScopeChoice::of(body.scope()));
}

/// Publishes a body over the entry the panel showed, recorded in the session's activity; the screens that show the
/// instructions read them again.
fn save(ctx: &mut Ctx, project: &OpenProject) -> FutureAction<(InstructionEntry, Instruction), Stored, Message> {
  let (project, services) = (project.clone(), Services::of(ctx));
  ctx.future_action(move |(entry, body): (InstructionEntry, Instruction)| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      let next = entry
        .state
        .published()
        .map_or(1, |version| version.get().saturating_add(1));
      let activity = services
        .activity
        .start(operations::instruction_publish(&project, entry.id, &entry.name, next));
      let root = project.repo_root.clone();
      let saved =
        background::run(move || editor::save_and_publish_in(&root, &entry, body).map_err(|error| error.message()))
          .await
          .and_then(|saved| saved);
      match &saved {
        Ok(_) => activity.finish(),
        Err(error) => activity.fail(error.clone()),
      }
      let saved = saved?;
      services.changes.record(project.project.id(), Outside::Instructions);
      Ok(Stored::published(saved))
    }
  })
}

/// Throws away the draft `entry` holds, while it still is the stored one, and reads the instruction then.
fn cancel(ctx: &mut Ctx, project: &OpenProject) -> FutureAction<InstructionEntry, Stored, Message> {
  let (project, services) = (project.clone(), Services::of(ctx));
  ctx.future_action(move |entry: InstructionEntry| {
    let (project, services) = (project.clone(), services.clone());
    async move {
      let root = project.repo_root.clone();
      let stored = background::run(move || {
        removal::discard_in::<Instruction>(&root, entry.id, &entry.body)?;
        entry_in(&root, entry.id).map_err(|error| messages::store_error(&error))
      })
      .await??;
      services.changes.record(project.project.id(), Outside::Instructions);
      Ok(Stored::published(stored))
    }
  })
}

/// What the scope's settings lack (a role, a pipeline or a step that is gone), under the section.
fn settings_error(ctx: &Ctx, errors: Option<&FormErrors>) -> Option<Element> {
  let error = errors?
    .settings
    .filter(|error| *error != SettingsError::TargetsRequired)?;
  Some(
    field_error(
      &format!("{PREFIX}-settings-error"),
      &message_text(ctx, &error.message()),
    )
    .into(),
  )
}
