//! Editor fields qSn5T: name, scope, agents, injection timing and prompt.
use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{BorderSize, FormTextRole, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, InputEditError, InputToken, InputTokens, Row, ScrollVertical, Select, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{Instruction, Variable};
use orchester_workspace::instructions::{
  data::{InstructionEntry, ProjectInstructions},
  form::{Events, FormErrors, InstructionForm, ScopeChoice, Target},
};

use super::{state::Tab, words};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, IconSize, instructions},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field, field_error, select_style},
    icon::{Icon, icon},
    message::message_text,
    prompt_input::{EditFeedback, PromptInput},
  },
};

// Prompt toolbar Un13s and line number gutter blqcd.
const PROMPT_TOOLBAR_HEIGHT: f32 = 34.0;
const GUTTER_WIDTH: f32 = 36.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingVariables(pub Vec<Variable>);
impl DevtoolsInspectable for MissingVariables {}

#[derive(Clone)]
pub struct Fields {
  pub name: Signal<String>,
  pub text: Signal<String>,
  pub input: PromptInput,
  pub unavailable: Signal<MissingVariables>,
  pub targets: Signal<Vec<Target>>,
  pub events: Signal<Events>,
  pub scope: Signal<ScopeChoice>,
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
    let before = std::sync::Arc::new(parking_lot::Mutex::new(targets.get_untracked()));
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
    Self {
      input,
      unavailable: ctx.signal(MissingVariables(Variable::ALL.to_vec())),
      name: ctx.signal(entry.map_or_else(String::new, |e| e.name.to_string())),
      text,
      targets,
      scope: ctx.signal(scope),
      events: ctx.signal(entry.map_or(Events::Start, |e| Events::of(e.body.events()))),
    }
  }
  pub fn reset(&self, entry: &InstructionEntry) {
    self.name.set(entry.name.to_string());
    self.text.set(entry.body.text().as_str().to_owned());
    self.targets.set(Target::of(entry.body.targets()));
    self.events.set(Events::of(entry.body.events()));
    self.scope.set(ScopeChoice::of(entry.body.scope()));
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
    if existing {
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
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::End)
      .with_children(self.scope_fields(ctx, data))
      .with_children((!matches!(self.scope.get(), ScopeChoice::AgentRole(_))).then(|| self.targets_field(ctx, data)))
      .child(
        Column::new().flex(1.0).child(field(
          "instruction-event-field",
          Some(&ctx.t("instructions.inject")),
          SelectField::new(
            "instruction-event",
            ctx.t("instructions.inject"),
            self.events.clone(),
            Events::ALL
              .into_iter()
              .map(|event| SelectOption::new(event, words::event(ctx, event)))
              .collect(),
          )
          .render(ctx),
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
          .and_then(|e| e.settings.as_ref())
          .map(|error| field_error("instruction-settings-error", &message_text(ctx, error))),
      )
      .into()
  }
  fn scope_fields(&self, ctx: &Ctx, data: &ProjectInstructions) -> Vec<Element> {
    let scope = self.scope.get();
    let control = |id: &str, key: &str, options| {
      Column::new()
        .flex(1.0)
        .child(field(
          id,
          Some(&ctx.t(key)),
          SelectField::new(id, ctx.t(key), self.scope.clone(), options)
            .placeholder(ctx.t("instructions.choose"))
            .render(ctx),
          None,
        ))
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
            .map(|s| SelectOption::new(ScopeChoice::Step(pipeline, Some(s.key.clone())), s.name.clone()))
            .collect()
        });
        vec![
          control("instruction-pipeline", "instructions.pipeline", pipelines),
          control("instruction-step", "instructions.step", steps),
        ]
      }
    }
  }
  fn targets_field(&self, ctx: &Ctx, data: &ProjectInstructions) -> Element {
    let mut options = vec![SelectOption::new(Target::All, ctx.t("instructions.all"))];
    options.extend(
      data
        .roles()
        .into_iter()
        .map(|role| SelectOption::new(Target::Role(role.clone()), role.as_str())),
    );
    options.extend(data.profiles.iter().map(|p| {
      SelectOption::new(
        Target::Profile(p.id),
        ctx.t_args(
          "instructions.profileLabel",
          [("role", p.role.as_str()), ("slug", &p.slug)],
        ),
      )
    }));
    let placeholder = ctx.t("instructions.addRole");
    let select = Select::multiple(self.targets.clone())
      .options(options)
      .style(select_style(ctx))
      .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .width(Dimension::Pct(100.0))
      .trigger(move |state| {
        let label = if state.selected_labels.is_empty() {
          placeholder.to_string()
        } else {
          state.selected_labels.join(", ")
        };
        Row::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::Center)
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&label)
              .variant(TypographyStyle::Description)
              .color(PaletteColor::TextPrimary)
              .flex(1.0),
          )
          .child(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::TextMuted))
      });
    Column::new()
      .flex(1.0)
      .child(field(
        "instruction-targets-field",
        Some(&ctx.t("instructions.apply")),
        Element::from(select).id("instruction-targets"),
        None,
      ))
      .into()
  }
  pub fn prompt(
    &self,
    ctx: &Ctx,
    panel: Signal<super::panels::Panel>,
    errors: Option<&FormErrors>,
    locked: bool,
  ) -> Element {
    let theme = ctx.theme();
    let text_style = FormTextRole {
      typography: instructions::prompt_text(),
      color: PaletteColor::TextSecondary,
    }
    .resolve(&theme.typography(), &theme.palette());
    let raw = self.text.get();
    let mut input = self
      .input
      .field(ctx, text_style, self.tokens(&raw))
      .id("instruction-prompt")
      .describe("a11y_name", ctx.t("instructions.prompt"))
      .rows(1, usize::MAX)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()));
    if locked {
      input = input
        .on_input(|event: lurq::app::events::TextInputEvent| event.prevent_default())
        .focusable(false);
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
              Text::new(
                &(1..=self.text.get().split('\n').count())
                  .map(|n| n.to_string())
                  .collect::<Vec<_>>()
                  .join("\n"),
              )
              .id("instruction-line-numbers")
              .variant(instructions::gutter_text())
              .width(Dimension::Px(GUTTER_WIDTH))
              .padding(Padding::new().top(ExtraSpacing::Loose.role()).right(SpacingSize::Sm)),
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
  fn tokens(&self, raw: &str) -> InputTokens {
    let unavailable = self.unavailable.get().0;
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
  fn prompt_toolbar(&self, ctx: &Ctx, panel: Signal<super::panels::Panel>) -> Row {
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
        .icon(Icon::CodeXml)
        .on_click(move || panel.set(super::panels::Panel::Variables)),
      )
  }
}
