//! Editor fields qSn5T: name, scope, agents, injection timing and prompt.
use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use lurq::{
  app::{
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{BorderSize, FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, InputEditError, InputToken, InputTokens, Row, ScrollVertical, Select, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{Instruction, PromptText, Variable};
use orchester_workspace::instructions::{
  data::{InstructionEntry, ProjectInstructions},
  form::{Events, FormErrors, InstructionForm, ScopeChoice, Target},
  publication::{self, Unavailable},
};

use super::{state::Tab, words, work::Author};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::{FieldInput, SelectField, field, field_error, select_style},
    icon::{Icon, icon},
    message::message_text,
    prompt_input::{EditFeedback, PromptInput},
  },
};

// Prompt toolbar Un13s and line number gutter blqcd.
const PROMPT_TOOLBAR_HEIGHT: f32 = 34.0;
const GUTTER_WIDTH: f32 = 36.0;
// Inject at fields in XMlEH, udfuB and w20Jf.
const PROJECT_EVENT_WIDTH: f32 = 180.0;
const ROLE_EVENT_WIDTH: f32 = 220.0;
const STEP_EVENT_WIDTH: f32 = 150.0;

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
  pub fn settings(
    &self,
    ctx: &mut Ctx,
    data: &ProjectInstructions,
    existing: bool,
    errors: Option<&FormErrors>,
  ) -> Element {
    let mut name = FieldInput::new("instruction-name", ctx.t("instructions.name"), self.name.clone());
    if existing || self.frozen.get() {
      name = name.locked();
    } else if !self.name.get().is_empty()
      && let Err(error) = orchester_core::name::Name::new(&self.name.get())
    {
      name = name.error(message_text(ctx, &orchester_workspace::messages::name_error(&error)));
    }
    let name = field(
      "instruction-name-field",
      Some(&ctx.t("instructions.name")),
      name.render(ctx),
      None,
    );
    // A new rule chooses its scope above these fields ([`super::scope_choice`]).
    let scope_fields = if existing { self.scope_fields(ctx, data) } else { Vec::new() };
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::End)
      .with_children(scope_fields)
      .with_children(
        (!matches!(self.scope.get(), ScopeChoice::AgentRole(_) | ScopeChoice::Step(..)))
          .then(|| self.targets_field(ctx, data, errors.is_some_and(FormErrors::targets_invalid))),
      )
      .child(
        Column::new()
          .width(Dimension::Px(match self.scope.get() {
            ScopeChoice::AgentRole(_) => ROLE_EVENT_WIDTH,
            ScopeChoice::Step(..) => STEP_EVENT_WIDTH,
            _ => PROJECT_EVENT_WIDTH,
          }))
          .child(field(
            "instruction-event-field",
            Some(&ctx.t("instructions.inject")),
            self.event_field(ctx),
            None,
          )),
      );
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Inset.role())
      .child(name)
      .child(row)
      .with_children(
        errors
          .and_then(|e| e.settings)
          .map(|error| field_error("instruction-settings-error", &message_text(ctx, &error.message()))),
      )
      .into()
  }
  fn event_field(&self, ctx: &Ctx) -> Element {
    let mut select = SelectField::new(
      "instruction-event",
      ctx.t("instructions.inject"),
      self.events.clone(),
      Events::ALL
        .into_iter()
        .map(|event| {
          SelectOption::new(
            event,
            words::scoped_event(ctx, event, matches!(self.scope.get(), ScopeChoice::AgentRole(_))),
          )
          .disabled(self.frozen.get())
        })
        .collect(),
    );
    if self.frozen.get() {
      select = select.locked();
    }
    select.render(ctx)
  }
  fn scope_fields(&self, ctx: &Ctx, data: &ProjectInstructions) -> Vec<Element> {
    let scope = self.scope.get();
    let control = |id: &str, key: &str, options: Vec<SelectOption<ScopeChoice>>| {
      let options = options
        .into_iter()
        .map(|option| option.disabled(self.frozen.get()))
        .collect();
      let mut select =
        SelectField::new(id, ctx.t(key), self.scope.clone(), options).placeholder(ctx.t("instructions.choose"));
      if self.frozen.get() {
        select = select.locked();
      }
      Column::new()
        .flex(1.0)
        .child(field(id, Some(&ctx.t(key)), select.render(ctx), None))
        .into()
    };
    match scope {
      ScopeChoice::Project => Vec::new(),
      ScopeChoice::AgentRole(_) => vec![control(
        "instruction-role",
        "instructions.role",
        data
          .roles()
          .into_iter()
          .map(|role| SelectOption::new(ScopeChoice::AgentRole(Some(role.clone())), role.as_str()))
          .collect(),
      )],
      ScopeChoice::Pipeline(_) => vec![control(
        "instruction-pipeline",
        "instructions.pipeline",
        data
          .pipelines
          .iter()
          .map(|p| SelectOption::new(ScopeChoice::Pipeline(Some(p.id)), p.name.clone()))
          .collect(),
      )],
      ScopeChoice::Step(pipeline, ref step) => {
        let pipelines = data
          .pipelines
          .iter()
          .map(|p| {
            SelectOption::new(
              ScopeChoice::Step(Some(p.id), if Some(p.id) == pipeline { step.clone() } else { None }),
              p.name.clone(),
            )
          })
          .collect();
        let steps = pipeline.and_then(|id| data.pipeline(id)).map_or_else(Vec::new, |p| {
          p.steps
            .iter()
            .map(|s| {
              SelectOption::new(
                ScopeChoice::Step(pipeline, Some(s.key.clone())),
                ctx.t_args(
                  "instructions.stepLabel",
                  [("number", format!("{:02}", s.ordinal)), ("name", s.name.to_string())],
                ),
              )
            })
            .collect()
        });
        vec![
          control("instruction-pipeline", "instructions.pipeline", pipelines),
          control("instruction-step", "instructions.step", steps),
        ]
      }
    }
  }
  fn targets_field(&self, ctx: &Ctx, data: &ProjectInstructions, invalid: bool) -> Element {
    let choices = target_choices(ctx, data);
    let remove_labels = choices
      .iter()
      .map(|(target, label)| {
        (
          label.to_string(),
          (
            target.clone(),
            ctx.t_args("instructions.removeTarget", [("name", label.as_ref())]),
          ),
        )
      })
      .collect::<std::collections::BTreeMap<_, _>>();
    let options = choices
      .into_iter()
      .map(|(target, label)| SelectOption::new(target, label));
    let placeholder = ctx.t("instructions.addRole");
    let targets = self.targets.clone();
    let frozen = self.frozen.clone();
    let select = Select::multiple(self.targets.clone())
      .options(options.map(|option| option.disabled(self.frozen.get())))
      .style(select_style(ctx))
      .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .width(Dimension::Pct(100.0))
      .trigger(move |state| {
        let mut chips = Row::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::Center)
          .spacing(SpacingSize::Xs);
        for (index, label) in state.selected_labels.iter().enumerate() {
          let mut chip = TagChip::new(format!("instruction-target-{index}"), label.as_ref());
          if !frozen.get()
            && let Some((target, remove_label)) = remove_labels.get(label.as_ref()).cloned()
          {
            let targets = targets.clone();
            chip = chip.removable(remove_label, move || {
              targets.update(|values| values.retain(|value| *value != target));
            });
          }
          chips = chips.child(chip);
        }
        chips
          .child(Text::new(&placeholder).variant(TypographyStyle::Caption).flex(1.0))
          .child(icon(Icon::Bot, IconSize::Md, PaletteColor::TextMuted))
      });
    Column::new()
      .flex(1.0)
      .child(field(
        "instruction-targets-field",
        Some(&ctx.t("instructions.apply")),
        Row::new()
          .width(Dimension::Pct(100.0))
          .opacity(if self.frozen.get() {
            crate::theme::DISABLED_OPACITY
          } else {
            1.0
          })
          .border_inside(
            BorderSize::Sm,
            if invalid {
              PaletteColor::Danger
            } else {
              PaletteColor::Border
            },
          )
          .rounded(ExtraRadius::Tile.role())
          .child(Element::from(select).id("instruction-targets")),
        None,
      ))
      .into()
  }
  pub fn prompt(
    &self,
    ctx: &Ctx,
    panel: super::state::SidePanel,
    errors: Option<&FormErrors>,
    (locked, unavailable): (bool, &[Variable]),
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
    let toolbar = self.prompt_toolbar(ctx, panel);
    Column::new()
      .id("instruction-prompt-editor")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .background(PaletteColor::SurfaceInput)
      .rounded(ExtraRadius::Tile.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .child(toolbar)
      .child(
        ScrollVertical::new(
          Row::new()
            .width(Dimension::Pct(100.0))
            .child(
              Row::new()
                .width(Dimension::Px(GUTTER_WIDTH))
                .padding(Padding::new().top(ExtraSpacing::Loose.role()).right(SpacingSize::Sm))
                .justify(lurq::layout::layout_kind::Justify::End)
                .child(
                  Text::new(
                    &(1..=self.text.get().split('\n').count())
                      .map(|n| n.to_string())
                      .collect::<Vec<_>>()
                      .join("\n"),
                  )
                  .id("instruction-line-numbers")
                  .variant(instructions::gutter_text())
                  .nowrap(),
                ),
            )
            .child(input),
        )
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0)),
      )
      .with_children(
        errors
          .and_then(|e| e.text.as_ref())
          .map(|error| field_error("instruction-text-error", &message_text(ctx, error))),
      )
      .with_children(self.input_error(ctx))
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
  fn prompt_toolbar(&self, ctx: &Ctx, panel: super::state::SidePanel) -> Row {
    Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .height(Dimension::Px(PROMPT_TOOLBAR_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(Text::new(&ctx.t("instructions.prompt")).variant(TypographyStyle::FieldLabel))
      .child(
        Text::new(&ctx.t_args(
          "instructions.lines",
          [("count", self.text.get().lines().count().max(1).to_string())],
        ))
        .variant(TypographyStyle::Caption)
        .flex(1.0),
      )
      .child(
        ActionButton::new(
          "instruction-variables",
          ButtonKind::Ghost,
          ctx.t("instructions.variables"),
        )
        .icon(Icon::Braces)
        .small()
        .compact()
        .on_click(move || panel.choose(super::state::Panel::Variables)),
      )
  }
}
fn target_choices(ctx: &Ctx, data: &ProjectInstructions) -> Vec<(Target, std::sync::Arc<str>)> {
  let mut choices = vec![(Target::All, ctx.t("instructions.all"))];
  choices.extend(
    data
      .roles()
      .into_iter()
      .map(|role| (Target::Role(role.clone()), std::sync::Arc::from(role.as_str()))),
  );
  choices.extend(data.profiles.iter().map(|profile| {
    (
      Target::Profile(profile.id),
      ctx.t_args(
        "instructions.profileLabel",
        [("role", profile.role.as_str()), ("slug", &profile.slug)],
      ),
    )
  }));

  choices
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
