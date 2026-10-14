//! The rule editor's column (`LIOyl` in `yRKeF`): the rule's name and state (`xuoek`), where it
//! applies and whom it reaches when (`m77bU`), its prompt with the Variables action (`qLKD8`,
//! `n3YuE`) and what publishing changes (`e5WnR`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Select, Text, TextOverflow},
  core::SignalValue,
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::run::PromptPart;

use super::{copy, editor::InstructionEditor};
use crate::{
  instructions::{
    data::InstructionEntry,
    form::{Events, FormErrors, ScopeChoice, Target},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    field::{field_error, select_style, styled_input},
    icon::{Icon, IconSize, icon},
    message_text,
    status_label::status_label,
  },
};

/// Setting select height (`KRDy4`) and the rows the prompt grows between (`g2Tkg`).
const SELECT_HEIGHT: f32 = 36.0;
const PROMPT_ROWS: (usize, usize) = (18, 400);

impl InstructionEditor {
  pub(super) fn rule_editor(&self, ctx: &Ctx, live: &InstructionEntry, errors: Option<&FormErrors>) -> Element {
    let part = PromptPart::of(live.body.scope());
    let (glyph, state) = if live.state.has_draft() {
      (Icon::FilePenLine, "instructions.state.draft")
    } else {
      (Icon::Check, "instructions.state.published")
    };
    let status = ctx.t_args(
      state,
      [
        ("kind", &*copy::kind(ctx, part)),
        ("version", copy::draft_number(live.state).to_string().as_str()),
        (
          "published",
          live
            .state
            .published()
            .map(|version| version.get().to_string())
            .unwrap_or_default()
            .as_str(),
        ),
      ],
    );
    let header = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .child(
        Text::new(&live.name)
          .variant(ExtraTypography::RuleTitle.role())
          .id("instruction-name"),
      )
      .child(Row::new().child(status_label(glyph, &status)));
    let mut settings = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(self.settings(ctx));
    if let Some(error) = errors.and_then(|errors| errors.settings) {
      settings = settings.child(field_error(&message_text(ctx, &error.message())));
    }
    Column::new()
      .id("instruction-editor")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Step.role())
      .child(header)
      .child(settings)
      .child(self.prompt(ctx, errors))
      .child(footer(ctx, live))
      .into()
  }

  /// "Apply to" and "Inject at", after the role, pipeline or step the rule applies in.
  fn settings(&self, ctx: &Ctx) -> Element {
    let scope = self.scope.get();
    let data = &self.context.data;
    let mut columns: Vec<Element> = Vec::new();
    match &scope {
      ScopeChoice::Project => {}
      ScopeChoice::AgentRole(_) => {
        let options = data
          .roles()
          .into_iter()
          .map(|role| (ScopeChoice::AgentRole(Some(role.clone())), Arc::from(role.as_str())))
          .collect();
        columns.push(self.scope_select(ctx, "instructions.settings.role", options));
      }
      ScopeChoice::Pipeline(_) => {
        let options = data
          .pipelines
          .iter()
          .map(|pipeline| (ScopeChoice::Pipeline(Some(pipeline.id)), pipeline.name.clone()))
          .collect();
        columns.push(self.scope_select(ctx, "instructions.settings.pipeline", options));
      }
      ScopeChoice::Step(..) => {
        let options = data
          .pipelines
          .iter()
          .flat_map(|pipeline| {
            pipeline.steps.iter().map(|step| {
              let label = ctx.t_args(
                "instructions.pipelineStep",
                [("pipeline", &*pipeline.name), ("step", &*step.name)],
              );
              (ScopeChoice::Step(Some(pipeline.id), Some(step.key.clone())), label)
            })
          })
          .collect();
        columns.push(self.scope_select(ctx, "instructions.settings.step", options));
      }
    }
    if !matches!(scope, ScopeChoice::AgentRole(_)) {
      columns.push(self.targets_select(ctx));
    }
    let events: Vec<(Events, Arc<str>)> = Events::ALL
      .into_iter()
      .map(|events| (events, events_label(ctx, events)))
      .collect();
    columns.push(setting(
      ctx,
      "instructions.settings.injectAt",
      "instruction-inject-at",
      sized(ctx, Select::new(self.events.clone()).options(events)),
    ));
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .with_children(columns)
      .into()
  }

  fn scope_select(&self, ctx: &Ctx, label: &str, options: Vec<(ScopeChoice, Arc<str>)>) -> Element {
    let select = Select::new(self.scope.clone())
      .options(options)
      .placeholder(ctx.t("instructions.settings.choose"));
    setting(ctx, label, "instruction-scope", sized(ctx, select))
  }

  /// "Apply to" (`KRDy4`): every agent, roles, or single profiles; the trigger lists the choices.
  fn targets_select(&self, ctx: &Ctx) -> Element {
    let data = &self.context.data;
    let mut options: Vec<(Target, Arc<str>)> = vec![(Target::All, ctx.t("instructions.targets.all"))];
    options.extend(
      data
        .roles()
        .into_iter()
        .map(|role| (Target::Role(role.clone()), Arc::from(role.as_str()))),
    );
    options.extend(data.profiles.iter().map(|profile| {
      (
        Target::Profile(profile.id),
        Arc::from(copy::profile_label(ctx, profile)),
      )
    }));
    let placeholder = ctx.t("instructions.settings.choose");
    let typography = ctx.theme().form().input.text.typography;
    // The trigger lists every choice; lurq's own says "N selected", which is not translated.
    let select = Select::multiple(self.targets.clone())
      .options(options)
      .trigger(move |state| {
        let text = if state.selected_labels.is_empty() {
          placeholder.to_string()
        } else {
          state.selected_labels.join(", ")
        };
        trigger_row(&text, typography)
      });
    setting(
      ctx,
      "instructions.settings.applyTo",
      "instruction-apply-to",
      sized(ctx, select),
    )
  }

  /// "Prompt" with the Variables action, and the prompt (`qLKD8`, `n3YuE`).
  fn prompt(&self, ctx: &Ctx, errors: Option<&FormErrors>) -> Element {
    let open = self.variables_open.clone();
    let variables = Button::empty()
      .id("instruction-variables")
      .focusable(false)
      .cursor(CursorIcon::Pointer)
      .on_click(move |_| open.set(true))
      .child(Text::new(&ctx.t("instructions.variables")).variant(TypographyStyle::Mono));
    let toolbar = Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .justify(Justify::SpaceBetween)
      .child(Text::new(&ctx.t("instructions.prompt")).variant(ExtraTypography::FormLabel.role()))
      .child(variables);
    let input = styled_input(ctx, self.text.clone(), ExtraTypography::PromptText.role())
      .id("instruction-prompt")
      .textarea()
      .rows(PROMPT_ROWS.0, PROMPT_ROWS.1)
      .width(Dimension::Pct(100.0))
      .placeholder(&ctx.t("instructions.promptPlaceholder"));
    let error = errors.and_then(|errors| errors.text.as_ref());
    let area = ScrollVertical::new(
      // The design's first line starts 18 below the edge plus the half-leading of its 1.8 lines,
      // which lurq's trimmed line box leaves out.
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(ExtraSpacing::Divider.role())
        .padding_top(SpacingSize::Lg)
        .child(input),
    )
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(
      BorderSize::Sm,
      if error.is_some() {
        PaletteColor::BorderStrong
      } else {
        PaletteColor::Border
      },
    ))
    .rounded(ExtraRadius::Notice.role());
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .spacing(ExtraSpacing::Step.role())
      .child(toolbar)
      .child(area)
      .with_children(error.map(|error| field_error(&message_text(ctx, error))))
      .into()
  }
}

/// A select trigger (`ZGwlv`): `text` and the chevron.
pub(super) fn trigger_row(text: &str, typography: TypographyStyle) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(text)
        .variant(typography)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0),
    )
    .child(icon(Icon::ChevronDown, IconSize::Dismiss, PaletteColor::TextSecondary))
    .into()
}

fn events_label(ctx: &Ctx, events: Events) -> Arc<str> {
  ctx.t(match events {
    Events::Start => "instructions.events.start",
    Events::Resume => "instructions.events.resume",
    Events::StartAndResume => "instructions.events.startAndResume",
  })
}

fn sized<T>(ctx: &Ctx, select: Select<T>) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static + SignalValue,
  Vec<T>: SignalValue,
{
  select
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SELECT_HEIGHT))
    .style(select_style(ctx, SELECT_HEIGHT))
    .into()
}

/// A setting (`MSfw2`): its label over its select.
fn setting(ctx: &Ctx, label: &str, id: &'static str, control: Element) -> Element {
  Column::new()
    .id(id)
    .flex(1.0)
    .spacing(ExtraSpacing::Field.role())
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Caption))
    .child(control)
    .into()
}

/// What publishing changes (`e5WnR`).
fn footer(ctx: &Ctx, live: &InstructionEntry) -> Element {
  let text = match live.state.published() {
    Some(version) => ctx.t_args(
      "instructions.publishEffect",
      [("version", version.get().to_string().as_str())],
    ),
    None => ctx.t("instructions.publishEffectFirst"),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::History, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(ExtraTypography::Footnote.role()))
    .into()
}
