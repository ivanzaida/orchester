//! An instruction's fields: name, scope, agents, injection timing and prompt. The Rule section's rows are
//! [`super::rule`]'s; the prompt is its head and its area (`Prompt` `Avkvg` in 02b).
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{BorderSize, FormTextRole, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, InputEditError, InputToken, InputTokens, Row, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{Instruction, PromptText, Variable};
use orchester_workspace::instructions::{
  data::InstructionEntry,
  form::{Events, FormErrors, InstructionForm, ScopeChoice, Target},
  publication::{self, Unavailable},
};

use super::{state::Tab, work::Author};
use crate::{
  screens::hub::parts::{head, note},
  theme::instructions,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    prompt_input::{EditFeedback, PromptInput},
  },
};

/// The prompt area's least height, an empty new rule's (`o912q` in 02e).
const PROMPT_MIN_HEIGHT: f32 = 110.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
  name: String,
  text: String,
  targets: Vec<Target>,
  events: Events,
  scope: ScopeChoice,
}

#[derive(Clone)]
pub struct Fields {
  pub frozen: Signal<bool>,
  pub name: Signal<String>,
  pub text: Signal<String>,
  pub input: PromptInput,
  pub targets: Signal<Vec<Target>>,
  pub events: Signal<Events>,
  pub scope: Signal<ScopeChoice>,
  /// Whether an MCP client's synthetic input edited the fields since the last save: the next draft is automation's.
  automated: Arc<AtomicBool>,
}
impl Fields {
  pub fn new(ctx: &mut Ctx, tab: Tab, entry: Option<&InstructionEntry>) -> Self {
    let scope = entry.map_or_else(
      || match tab {
        Tab::Project => ScopeChoice::Project,
        Tab::Roles => ScopeChoice::AgentRole(None),
        Tab::Pipelines => ScopeChoice::Pipeline(None),
        Tab::Steps => ScopeChoice::Step(None, None),
      },
      |entry| ScopeChoice::of(entry.body.scope()),
    );
    let targets = ctx.signal(entry.map_or_else(|| vec![Target::All], |entry| Target::of(entry.body.targets())));
    let before = Arc::new(parking_lot::Mutex::new(targets.get_untracked()));
    let changed = targets.clone();
    ctx.watch(&targets, move |next| {
      let mut before = before.lock();
      if *before == *next {
        return;
      }
      let normalized = Target::after_change(&before, next.clone());
      *before = normalized.clone();
      if normalized != *next {
        changed.set(normalized);
      }
    });
    let text = ctx.signal(entry.map_or_else(String::new, |e| e.body.text().as_str().to_owned()));
    let input = PromptInput::new(ctx, text.clone());
    let fields = Self {
      frozen: ctx.signal(false),
      input,
      name: ctx.signal(entry.map_or_else(String::new, |e| e.name.to_string())),
      text,
      targets,
      scope: ctx.signal(scope),
      events: ctx.signal(entry.map_or(Events::Start, |e| Events::of(e.body.events()))),
      automated: Arc::new(AtomicBool::new(false)),
    };
    // Signals change while lurq delivers the input that changed them, so a client's edit is told from a person's.
    let automated = fields.automated.clone();
    let mark = move || {
      if is_synthetic_input() {
        automated.store(true, Ordering::SeqCst);
      }
    };
    let watched = mark.clone();
    ctx.watch(&fields.name, move |_| watched());
    let watched = mark.clone();
    ctx.watch(&fields.text, move |_| watched());
    let watched = mark.clone();
    ctx.watch(&fields.targets, move |_| watched());
    let watched = mark.clone();
    ctx.watch(&fields.events, move |_| watched());
    ctx.watch(&fields.scope, move |_| mark());
    fields
  }
  /// Who wrote the edits a save takes now: automation when an MCP client's input edited the fields since the last
  /// save started, or presses the button that saves.
  pub fn author(&self) -> Author {
    if self.automated.load(Ordering::SeqCst) || is_synthetic_input() {
      Author::Automation
    } else {
      Author::Person
    }
  }
  /// A save started: edits from now on are the next save's.
  pub fn saving(&self) {
    self.automated.store(false, Ordering::SeqCst);
  }
  pub fn snapshot(&self) -> Snapshot {
    Snapshot {
      name: self.name.get_untracked(),
      text: self.text.get_untracked(),
      targets: self.targets.get_untracked(),
      events: self.events.get_untracked(),
      scope: self.scope.get_untracked(),
    }
  }
  pub fn reload(&self, entry: &InstructionEntry, requested: &Snapshot) {
    let current = self.snapshot();
    if current.name == requested.name {
      self.name.set(entry.name.to_string());
    }
    if current.text == requested.text {
      self.text.set(entry.body.text().as_str().to_owned());
    }
    if current.targets == requested.targets {
      self.targets.set(Target::of(entry.body.targets()));
    }
    if current.events == requested.events {
      self.events.set(Events::of(entry.body.events()));
    }
    if current.scope == requested.scope {
      self.scope.set(ScopeChoice::of(entry.body.scope()));
    }
  }
  /// The prompt's variables without a value at one of the chosen events: publication waits for none.
  pub fn blocked(&self) -> Vec<Unavailable> {
    // A prompt that does not parse shows its own error and cannot be saved or published.
    PromptText::new(&self.text.get())
      .map(|text| publication::missing(&text, &self.events.get().set()))
      .unwrap_or_default()
  }
  pub fn instruction(&self) -> Result<Instruction, FormErrors> {
    InstructionForm {
      text: &self.text.get(),
      targets: &self.targets.get(),
      events: self.events.get(),
      scope: &self.scope.get(),
    }
    .instruction()
  }
  /// The prompt's head and area; a saved rule's has the note on missing and secret values under it (`fSC70` in 02b).
  pub fn prompt(
    &self,
    ctx: &Ctx,
    panel: super::state::SidePanel,
    errors: Option<&FormErrors>,
    (locked, unavailable, saved): (bool, &[Variable], bool),
  ) -> Element {
    let theme = ctx.theme();
    let text_style = FormTextRole {
      typography: instructions::prompt_text(),
      color: PaletteColor::TextSecondary,
    }
    .resolve(&theme.typography(), &theme.palette());
    let raw = self.text.get();
    let frozen = self.frozen.clone();
    let mut input = self
      .input
      .field(ctx, text_style, tokens(&raw, unavailable))
      .id("instruction-prompt")
      .describe("a11y_name", ctx.t("instructions.prompt"))
      .rows(1, usize::MAX)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::symmetric(SpacingSize::Md, instructions::prompt_top()))
      .on_input(move |event: lurq::app::events::TextInputEvent| {
        if locked || frozen.get_untracked() {
          event.prevent_default();
        }
      });
    if locked || self.frozen.get() {
      input = input.focusable(false);
    }
    let area = Column::new()
      .id("instruction-prompt-editor")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(PROMPT_MIN_HEIGHT))
      .background(PaletteColor::SurfaceInput)
      .rounded(RadiusSize::Md)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .child(
        ScrollVertical::new(Row::new().width(Dimension::Pct(100.0)).child(input))
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0)),
      );
    Column::new()
      .id("instruction-prompt-section")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .spacing(SpacingSize::Sm)
      .child(self.prompt_head(ctx, panel))
      .child(area)
      .with_children(
        errors
          .and_then(|e| e.text.as_ref())
          .map(|error| field_error("instruction-text-error", &message_text(ctx, error))),
      )
      .with_children(self.input_error(ctx))
      .with_children(saved.then(|| note("instruction-prompt-note", Icon::Info, &ctx.t("instructions.promptNote"))))
      .into()
  }
  fn input_error(&self, ctx: &Ctx) -> Option<Element> {
    let feedback = self.input.feedback.get();
    let key = match &feedback {
      EditFeedback::Clear => return None,
      EditFeedback::Prevented => "instructions.insertPrevented",
      EditFeedback::Failed(error) => match **error {
        InputEditError::NotMounted => "instructions.insertNotMounted",
        InputEditError::Masked => "instructions.insertMasked",
        InputEditError::Composing => "instructions.insertComposing",
        InputEditError::InvalidSelection => "instructions.insertInvalidSelection",
      },
    };
    Some(field_error("instruction-insert-error", &ctx.t(key)).into())
  }
  /// *Prompt*, then *Markdown · N lines* (only *Markdown* while it is empty), and Insert variable, which opens the
  /// Variables panel (`yTbU8` in 02b, `Q6O9YA` in 02e).
  fn prompt_head(&self, ctx: &Ctx, panel: super::state::SidePanel) -> Row {
    let text = self.text.get();
    let kind = match text.lines().count() {
      _ if text.is_empty() => ctx.t("instructions.markdown"),
      0 | 1 => ctx.t("instructions.linesOne"),
      count => ctx.t_args("instructions.lines", [("count", count.to_string())]),
    };
    let insert = ActionButton::new(
      "instruction-variables",
      ButtonKind::Ghost,
      ctx.t("instructions.insertVariable"),
    )
    .icon(Icon::Braces)
    .on_click(move || panel.choose(super::state::Panel::Variables));
    head(
      "instruction-prompt",
      &ctx.t("instructions.prompt"),
      Some(&kind),
      Some(insert.into()),
    )
  }
}
/// The prompt's variables as pills, those without a value in the preview marked.
fn tokens(raw: &str, unavailable: &[Variable]) -> InputTokens {
  let tokens = orchester_workspace::instructions::tokens::inline_references(raw)
    .into_iter()
    .map(|reference| {
      InputToken::new(
        reference.range,
        instructions::token_style(unavailable.contains(&reference.variable)),
      )
    })
    .collect();
  InputTokens::new(raw, tokens).expect("domain ranges are nonoverlapping UTF-8 boundaries without line endings")
}
