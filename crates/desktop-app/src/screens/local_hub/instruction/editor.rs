//! The editor of a local instruction's panel: the form that makes one ([`InstructionFields`]) over its published
//! version, and the pinned *Save* and *Cancel*. Typing changes only the fields. *Save* publishes what they hold as the
//! next version at once ([`editor::save_and_publish_in`]), over a draft stored meanwhile too, under the person's
//! consent when an MCP client presses it; *Cancel* puts the published version back in the fields. A refusal says why in
//! place and keeps what was typed.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::resource::Instruction;
use orchester_workspace::{
  activity::operations,
  background,
  instructions::{data::InstructionEntry, editor},
  local_archive::Archivable,
  messages::Message,
  tasks::outside::Outside,
  workspace::OpenProject,
};

use super::{PREFIX, Props, UnsavedSignal};
use crate::{
  mcp::input_gate::{Guarded, InputGate, may_press},
  screens::{
    hub::forms::{Form, Ids, InstructionFields},
    local_hub::{
      leave::{clear_unsaved, mark_unsaved},
      panel::{chips_of, frame, head_end, readiness, standing_chips, used_by},
      save_bar::{self, Save},
      words::kind_where_key,
    },
    team_hub::{
      panel::{body, given_by, head_named},
      rows,
    },
  },
  services::Services,
  ui::{field::field_error, message::message_text, pressable::Press},
};

/// The ids of the form's parts, which tests and automation address: `local-hub-instruction-format`,
/// `local-hub-instruction-text-input`, …
const IDS: Ids = Ids {
  rows: PREFIX,
  sections: PREFIX,
};

#[derive(Clone)]
pub(super) struct EditorProps {
  pub props: Props,
  /// The instruction as it was read when the panel opened, with a draft stored before if it has one: what a save is
  /// checked against. The editor keeps what it stores itself from then on.
  pub entry: InstructionEntry,
  /// Its latest published version, which the fields start from and are compared with; its draft while it has only
  /// one.
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
  fields: InstructionFields,
  stored: Signal<Stored>,
  /// Publishes what the fields hold over the stored entry.
  save: FutureAction<(InstructionEntry, Instruction), Stored, Message>,
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
    let fields = InstructionFields::new(ctx);
    fields.load(&published);
    let save = save(ctx, &props.project.open);
    let saved = ctx.signal(Stored { entry, published });
    let stored = saved.clone();
    ctx.watch(&save.state(), move |state| {
      if let Some(now) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        saved.set(now.clone());
      }
    });
    Self {
      fields,
      stored,
      save,
      gate: ctx.use_context::<InputGate>(),
      unsaved: props.unsaved,
      what: props.entry.what,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps { props, .. } = ctx.props::<EditorProps>().clone();
    let stored = self.stored.get();
    let saving = self.save.state().get();
    let busy = saving.is_pending();
    let drawn = self.fields.draw(
      ctx,
      &Form {
        ids: IDS,
        base: Some(&stored.published),
        name: None,
        tried: true,
        locked: busy,
      },
    );
    let differs = drawn.made.as_ref().map_or(true, |body| *body != stored.published);
    // An instruction that has only a draft so far has it to publish: what it holds is not out yet.
    let changed = differs || stored.entry.state.published().is_none();
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
    let rows = std::iter::once(used_by(ctx, PREFIX, entry)).chain(drawn.rows).collect();
    let body = body()
      .child(head)
      .child(chips_of(
        standing_chips(ctx, entry),
        given_by(
          ctx,
          ("local_hub.panel.givenBy.one", "local_hub.panel.givenBy.other"),
          entry.used_by.len(),
        ),
      ))
      .child(rows::section(
        PREFIX,
        &ctx.t("local_hub.instruction.title"),
        Vec::new(),
        rows,
      ))
      .with_children(drawn.sections)
      .with_children(
        refused
          .map(|error| -> Element { field_error(&format!("{PREFIX}-refused"), &message_text(ctx, &error)).into() }),
      )
      .with_children(readiness(ctx, entry));
    let bar = save_bar::bar(
      ctx,
      PREFIX,
      (
        self.save_now(&stored, drawn.made, (changed, busy)),
        (differs && !busy).then(|| self.cancel_now(&stored)),
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
  /// What *Save* does now: nothing while nothing differs, why it waits while the fields make no instruction, else it
  /// publishes what they make.
  fn save_now(&self, stored: &Stored, made: Result<Instruction, Arc<str>>, (changed, busy): (bool, bool)) -> Save {
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
    let (save, gate, entry) = (self.save.clone(), self.gate.clone(), stored.entry.clone());
    Save::Ready(Press::run(move || {
      if may_press(
        gate.as_ref(),
        Guarded::PermissionWidening,
        "publish an instruction, which reaches agents' prompts",
      ) {
        save.run((entry.clone(), body.clone()));
      }
    }))
  }

  /// *Cancel*: the fields take the published version at once. A draft stored meanwhile is left as it is: the panel
  /// never showed it.
  fn cancel_now(&self, stored: &Stored) -> Press {
    let (fields, published) = (self.fields.clone(), stored.published.clone());
    Press::run(move || fields.load(&published))
  }
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
