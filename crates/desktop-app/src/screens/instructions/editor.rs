//! An instruction's page (`02b Instruction` `Sn37u`, `02e New instruction` `dMYPx`): its head and draft bar, the rule's
//! settings and prompt, and the Prompt preview / Variables panel beside them. Edits save as a draft once the fields
//! rest ([`super::autosave`]); a new rule chooses its scope and is created with Create draft.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{name::Name, resource::Instruction};
use orchester_workspace::{
  agents::data::VersionState,
  instructions::{
    data::{InstructionEntry, ProjectInstructions},
    form::FormErrors,
    preview::PreviewChoice,
    publication,
  },
};

use super::{
  autosave::Autosave,
  availability::{self, Availability},
  editor_actions::{self, Ask},
  fields::Fields,
  page_head, panels, scope_choice,
  state::{Panel, Saved, Selection, SidePanel, Tab, View},
  words,
  work::{self, Author, Change},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  project::ShownProject,
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// Whether the input being delivered may publish an instruction (ORC-30): a person's always; an MCP client's only
/// under *Let automation change agent permissions*, as `publish_instruction` takes it.
pub(super) fn may_publish(gate: Option<&InputGate>) -> bool {
  let may = input_may_decide(gate, Guarded::PermissionWidening);
  if !may {
    tracing::warn!("synthetic input pressed Publish on an instruction; not taken");
  }
  may
}

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub data: ProjectInstructions,
  pub entry: Option<InstructionEntry>,
  pub tab: Tab,
  /// The panel the route shows beside the rule.
  pub panel: Panel,
  pub selection: Selection,
  /// Counts the reloads, each of which reads the list again.
  pub reloads: Signal<u64>,
  pub reading: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.data == other.data
      && self.entry == other.entry
      && self.tab == other.tab
      && self.panel == other.panel
      && self.reading == other.reading
  }
}
impl DevtoolsInspectable for Props {}
pub struct Editor {
  pub(super) fields: Fields,
  pub(super) saved: Signal<Saved>,
  pub(super) work: work::Work,
  autosave: Autosave,
  /// What the preview is for: the panels choose it, the editor reads what it says about the variables.
  choice: Signal<PreviewChoice>,
  pub(super) panel: SidePanel,
  pub(super) published: Signal<Option<u32>>,
  /// Publishing reaches agents' prompts: lurq's synthetic input publishes only under the person's consent.
  pub(super) gate: Option<InputGate>,
  pub(super) ask: Signal<Ask>,
  pub(super) more: Signal<bool>,
  pub(super) anchor: ElementRef,
}
impl Component for Editor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let fields = Fields::new(ctx, props.tab, props.entry.as_ref());
    let saved = ctx.signal(Saved(props.entry.clone()));
    let work = work::action(ctx, &props.project.open, &fields);
    let published = ctx.signal(None);
    let autosave = Autosave::new(ctx, &fields, saved.clone(), work.clone(), published.clone());
    let editor = Self {
      fields: fields.clone(),
      saved: saved.clone(),
      work,
      autosave: autosave.clone(),
      choice: ctx.signal(PreviewChoice::default()),
      panel: SidePanel::new(ctx.signal(props.panel), props.selection.clone()),
      published: published.clone(),
      gate: ctx.use_context::<InputGate>(),
      ask: ctx.signal(Ask::Nothing),
      more: ctx.signal(false),
      anchor: ctx.element_ref(),
    };
    let (selection, reloads) = (props.selection.clone(), props.reloads.clone());
    let fields = editor.fields.clone();
    ctx.watch(&editor.work.action.state(), move |result| {
      selection.busy.set(result.is_pending());
      if !result.is_pending() {
        fields.frozen.set(false);
      }
      if let Some(done) = result.data.as_ref().filter(|_| result.is_fulfilled()) {
        saved.set(Saved(Some(done.entry.clone())));
        if let Some(requested) = &done.reload {
          fields.reload(&done.entry, requested);
          reloads.update(|count| *count += 1);
        }
        if done.published {
          published.set(done.entry.state.published().map(|v| v.get()));
        }
        autosave.again();
        // Opening the newly created resource preserves the complete body just saved, and its panel.
        if let View::New(_, panel) = selection.now() {
          selection.created.set(Saved(Some(done.entry.clone())));
          selection.dirty.set(false);
          selection.saved(View::Rule(Tab::of(done.entry.body.scope()), Some(done.entry.id), panel));
        }
      }
    });
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    self.panel.follow(props.panel);
    let parsed = self.fields.instruction();
    let saved = self.saved.get().0;
    let dirty = parsed
      .as_ref()
      .map_or(true, |body| saved.as_ref().is_none_or(|entry| entry.body != *body));
    if props.selection.dirty.get_untracked() != dirty {
      props.selection.dirty.set(dirty);
    }
    let busy = self.work.action.state().get().is_pending();
    if props.selection.busy.get_untracked() != busy {
      props.selection.busy.set(busy);
    }
    // A plain cell, not a signal: the autosave reads it when its rest ends, and nothing renders from it.
    self.autosave.reading.set(props.reading);
    let live = saved.as_ref().map(|entry| InstructionEntry {
      body: parsed.as_ref().map_or_else(|_| entry.body.clone(), Clone::clone),
      state: if dirty {
        VersionState::Draft {
          published: entry.state.published(),
        }
      } else {
        entry.state
      },
      ..entry.clone()
    });
    let catalog = availability::catalog(ctx, &props.project, &props.data);
    let availability = Availability::of(ctx, &catalog, &self.choice.get(), live.as_ref());
    let pane = self.pane(ctx, &props, (&parsed, dirty, busy), &availability);
    let panel = ctx.mount_keyed::<panels::Panels>(
      "instruction-panels",
      panels::Props {
        project: props.project.clone(),
        entry: live,
        valid: parsed.is_ok(),
        input: self.fields.input.clone(),
        frozen: self.fields.frozen.clone(),
        events: self.fields.events.clone(),
        choice: self.choice.clone(),
        catalog,
        availability,
        panel: self.panel.clone(),
        data: props.data.clone(),
      },
    );
    if self.panel.get() == Panel::Full {
      return panel;
    }
    Row::new()
      .id("instruction-editor")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(HubSpacing::BodyTop.role())
          .left(HubSpacing::Side.role())
          .right(HubSpacing::Side.role())
          .bottom(HubSpacing::Row.role()),
      )
      .spacing(HubSpacing::Beside.role())
      .child(pane)
      .child(panel)
      .into()
  }
}
impl Editor {
  fn pane(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (parsed, dirty, busy): (&Result<Instruction, FormErrors>, bool, bool),
    availability: &Availability,
  ) -> Element {
    let errors = parsed.as_ref().err();
    let saved_value = self.saved.get().0;
    let saved = saved_value.as_ref();
    let top = self.top(ctx, props, saved, (dirty, busy, errors.is_some()));
    let mut pane = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(top);
    pane = editor_actions::write_error(self, ctx, pane);
    pane = editor_actions::published_notice(self, ctx, &availability.retained, pane);
    pane = editor_actions::reload(self, ctx, props, (saved, busy), pane);
    if saved.is_none() {
      pane = pane.child(scope_choice::render(ctx, &self.fields, &props.data, busy));
    }
    let settings = self.fields.settings(ctx, &props.data, saved.is_some(), errors);
    let prompt = self.fields.prompt(
      ctx,
      self.panel.clone(),
      errors,
      (busy && !dirty, &availability.unavailable),
    );
    pane
      .child(settings)
      .with_children(super::editor_notes::scope(ctx, &self.fields, &props.data))
      .child(prompt)
      .with_children(super::editor_notes::warning(ctx, &self.fields, availability))
      .into()
  }

  /// The crumb, the head, and the draft bar or what is asked in its place.
  fn top(&self, ctx: &mut Ctx, props: &Props, saved: Option<&InstructionEntry>, (dirty, busy, invalid): (bool, bool, bool)) -> Element {
    let name = self.fields.name.get();
    let title = if name.trim().is_empty() {
      ctx.t("instructions.new")
    } else {
      name.as_str().into()
    };
    let tab = saved.map_or(props.tab, |entry| Tab::of(entry.body.scope()));
    let sub = ctx.t_args(
      "instructions.pageSub",
      [
        ("scope", words::scope(ctx, tab).to_string()),
        ("project", props.project.open.project.name().to_string()),
      ],
    );
    let more = saved.map(|_| page_head::More {
      open: self.more.clone(),
      anchor: self.anchor.clone(),
      remove: {
        let ask = self.ask.clone();
        std::sync::Arc::new(move || ask.set(Ask::Remove))
      },
    });
    let state = saved.map_or(VersionState::Draft { published: None }, |e| e.state);
    let automated = !busy && !dirty && saved.is_some_and(|entry| props.data.by_automation(entry));
    let bar = match editor_actions::question(self, ctx, props, saved) {
      Some(question) => question,
      None => page_head::bar(
        ctx,
        tab,
        &page_head::Now {
          state,
          dirty,
          busy,
          invalid,
          automated,
          saved_at: saved.and_then(|entry| entry.draft_saved_at),
        },
        self.bar_actions(ctx, saved, dirty, busy),
      ),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(page_head::crumb(ctx, &props.selection, saved.is_none()))
      .child(page_head::head(ctx, (&title, &sub), more))
      .with_children(editor_actions::remove_question(self, ctx, props, saved))
      .child(bar)
      .into()
  }

  fn bar_actions(&self, ctx: &Ctx, saved: Option<&InstructionEntry>, dirty: bool, busy: bool) -> Vec<Element> {
    let mut actions = Vec::new();
    if let Some(entry) = saved
      && entry.state.has_draft()
      && entry.state.published().is_some()
    {
      let ask = self.ask.clone();
      let discard = ActionButton::new("instruction-discard", ButtonKind::Ghost, ctx.t("hub.discard.action")).icon(Icon::Undo2);
      actions.push(if busy || dirty { discard.disabled() } else { discard.on_click(move || ask.set(Ask::Discard)) }.into());
    }
    actions.push(self.action_button(ctx, saved, dirty, busy).into());
    actions
  }

  fn action_button(&self, ctx: &Ctx, saved: Option<&InstructionEntry>, dirty: bool, busy: bool) -> ActionButton {
    let state = saved.map_or(VersionState::Draft { published: None }, |e| e.state);
    let number = state.published().map_or(1, |v| v.get().saturating_add(1));
    let work = self.work.clone();
    let Some(entry) = saved else {
      let candidate = self
        .fields
        .instruction()
        .ok()
        .and_then(|body| Name::new(&self.fields.name.get()).ok().map(|name| (name, body)));
      let valid = candidate.is_some();
      let save = ActionButton::new("instruction-create", ButtonKind::Primary, ctx.t("instructions.createDraft"))
        .icon(Icon::Check)
        .on_click(move || {
          if let Some((name, body)) = &candidate {
            // Work::run tells whether an MCP client's input wrote it or presses Create.
            work.run(Change::Create(name.clone(), body.clone(), Author::Person).into());
          }
        });
      return if busy {
        save.loading()
      } else if valid {
        save
      } else {
        save.disabled()
      };
    };
    if !dirty && !state.has_draft() {
      let panel = self.panel.clone();
      return ActionButton::new("instruction-preview-prompt", ButtonKind::Secondary, ctx.t("instructions.previewPrompt"))
        .icon(Icon::Eye)
        .on_click(move || panel.choose(Panel::Preview));
    }
    let (entry, gate) = (entry.clone(), self.gate.clone());
    let publish = ActionButton::new(
      "instruction-publish",
      ButtonKind::Primary,
      ctx.t_args("instructions.publish", [("version", number.to_string())]),
    )
    .icon(Icon::Upload)
    .on_click(move || {
      if may_publish(gate.as_ref()) && publication::missing(entry.body.text(), entry.body.events()).is_empty() {
        work.run(Change::Publish(entry.clone()).into());
      }
    });
    if self.work.publishing() {
      publish.loading()
    } else if busy || dirty || !state.has_draft() || matches!(state, VersionState::StaleDraft { .. }) {
      publish.unavailable(ctx.t("instructions.publishSaveWaiting"))
    } else if !self.fields.blocked().is_empty() {
      publish.unavailable(ctx.t("instructions.publishWaiting"))
    } else {
      publish
    }
  }
}
