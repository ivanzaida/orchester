//! The Rule section of an instruction's page (`Rule` `G0e2W` in `02b Instruction` `Sn37u`, `qUldm` in `02e New
//! instruction` `dMYPx`): setting rows with their label and hint at the start and a 300 control at the end — the
//! name; a saved rule's role, pipeline or step (a new rule chooses them in [`super::scope_choice`]); who it applies to
//! where the scope names agents; and when it is injected, as Agent start · Agent resume · Start & resume.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, SelectOption, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::instructions::{
  data::ProjectInstructions,
  form::{Events, FormErrors, ScopeChoice, SettingsError, Target},
};

use super::{fields::Fields, words};
use crate::{
  screens::hub::parts::{boxed, field, group, head},
  theme::{ExtraRadius, IconSize},
  ui::{
    field::{FieldInput, SelectField, field_error, select_style},
    icon::{Icon, icon},
    message::message_text,
    pressable::Focus,
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// A setting's control at its row's end (`T4BnX`, `kHLEE` in `y5bKI`).
const CONTROL_WIDTH: f32 = 300.0;

impl Fields {
  /// The Rule section; `existing` for a saved rule, whose name is fixed and whose scope is chosen here.
  pub fn settings(
    &self,
    ctx: &mut Ctx,
    data: &ProjectInstructions,
    existing: bool,
    errors: Option<&FormErrors>,
  ) -> Element {
    let mut rows = vec![self.name_row(ctx, existing)];
    if existing {
      rows.extend(self.scope_rows(ctx, data));
    }
    let scope = self.scope.get();
    if !matches!(scope, ScopeChoice::AgentRole(_) | ScopeChoice::Step(..)) {
      let hint = ctx.t(if matches!(scope, ScopeChoice::Pipeline(_)) {
        "instructions.applyHintPipeline"
      } else {
        "instructions.applyHintProject"
      });
      let invalid = errors.is_some_and(FormErrors::targets_invalid);
      rows.push(
        field(
          "instruction-targets-field",
          &ctx.t("instructions.apply"),
          Some(&hint),
          self.targets_field(ctx, data, invalid),
        )
        .into(),
      );
    }
    let inject_hint = existing.then(|| ctx.t("instructions.injectHint"));
    rows.push(
      field(
        "instruction-event-field",
        &ctx.t("instructions.inject"),
        inject_hint.as_deref(),
        self.event_field(ctx, existing),
      )
      .into(),
    );
    // A new rule's role, pipeline or step is chosen in the Scope section above, whose select says it is not chosen
    // yet, and Create draft waits for it (02e).
    let shown = errors.and_then(|e| e.settings).filter(|error| {
      existing
        || !matches!(
          error,
          SettingsError::RoleRequired | SettingsError::PipelineRequired | SettingsError::StepRequired
        )
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(group(
        "instruction-rule",
        head("instruction-rule", &ctx.t("instructions.ruleTitle"), None, None),
        boxed("instruction-rule-rows", rows),
      ))
      .with_children(shown.map(|error| field_error("instruction-settings-error", &message_text(ctx, &error.message()))))
      .into()
  }

  fn name_row(&self, ctx: &mut Ctx, existing: bool) -> Element {
    let mut name = FieldInput::new("instruction-name", ctx.t("instructions.name"), self.name.clone())
      .placeholder(ctx.t("instructions.name"))
      .width(Dimension::Px(CONTROL_WIDTH));
    if existing || self.frozen.get() {
      name = name.locked();
    } else if !self.name.get().is_empty()
      && let Err(error) = orchester_core::name::Name::new(&self.name.get())
    {
      name = name.error(message_text(ctx, &orchester_workspace::messages::name_error(&error)));
    }
    field(
      "instruction-name-field",
      &ctx.t("instructions.name"),
      None,
      name.render(ctx),
    )
    .into()
  }

  /// Agent start · Agent resume · Start & resume; while the rule is being created the choice stays as it is. A saved
  /// role rule names its agents in the last one; a new rule's reads as drawn in 02e.
  fn event_field(&self, ctx: &Ctx, existing: bool) -> Element {
    let role = existing && matches!(self.scope.get(), ScopeChoice::AgentRole(_));
    let chosen = self.events.get();
    let segments = Events::ALL.into_iter().map(|event| {
      let (events, frozen) = (self.events.clone(), self.frozen.clone());
      let name = match event {
        Events::Start => "start",
        Events::Resume => "resume",
        Events::StartAndResume => "both",
      };
      Segment::new(
        format!("instruction-event-{name}"),
        SegmentFace::Text(words::scoped_event(ctx, event, role)),
        event == chosen,
        move || {
          if !frozen.get_untracked() {
            events.set(event);
          }
        },
      )
    });
    segmented("instruction-event", segments, Focus::Keyboard).into()
  }

  /// A saved rule's role, pipeline, or pipeline and step, each a row of its own.
  fn scope_rows(&self, ctx: &Ctx, data: &ProjectInstructions) -> Vec<Element> {
    let scope = self.scope.get();
    let row = |id: &str, key: &str, options: Vec<SelectOption<ScopeChoice>>| -> Element {
      let options = options
        .into_iter()
        .map(|option| option.disabled(self.frozen.get()))
        .collect();
      let mut select = SelectField::new(id, ctx.t(key), self.scope.clone(), options)
        .placeholder(ctx.t("instructions.choose"))
        .width(Dimension::Px(CONTROL_WIDTH));
      if self.frozen.get() {
        select = select.locked();
      }
      field(&format!("{id}-field"), &ctx.t(key), None, select.render(ctx)).into()
    };
    match scope {
      ScopeChoice::Project => Vec::new(),
      ScopeChoice::AgentRole(_) => vec![row(
        "instruction-role",
        "instructions.role",
        data
          .roles()
          .into_iter()
          .map(|role| SelectOption::new(ScopeChoice::AgentRole(Some(role.clone())), role.as_str()))
          .collect(),
      )],
      ScopeChoice::Pipeline(_) => vec![row(
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
          row("instruction-pipeline", "instructions.pipeline", pipelines),
          row("instruction-step", "instructions.step", steps),
        ]
      }
    }
  }

  /// All agents, or the roles and profiles it names, in one 300 select (`ofaBF` in `EsJ8r`): what it names on one
  /// line, then the chevron; its menu names more or fewer.
  fn targets_field(&self, ctx: &Ctx, data: &ProjectInstructions, invalid: bool) -> Element {
    let options = target_choices(ctx, data)
      .into_iter()
      .map(|(target, label)| SelectOption::new(target, label));
    let placeholder = ctx.t("instructions.choose");
    let select = Select::multiple(self.targets.clone())
      .options(options.map(|option| option.disabled(self.frozen.get())))
      .style(select_style(ctx))
      .height(Dimension::Px(crate::theme::ControlHeight::Md.value()))
      .width(Dimension::Pct(100.0))
      .trigger(move |state| {
        let named = !state.selected_labels.is_empty();
        let shown = if named {
          state
            .selected_labels
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<&str>>()
            .join(", ")
        } else {
          placeholder.to_string()
        };
        Row::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::Center)
          .spacing(SpacingSize::Sm)
          .child(
            Text::new(&shown)
              .id("instruction-targets-value")
              .variant(TypographyStyle::Description)
              .color(if named {
                PaletteColor::TextPrimary
              } else {
                PaletteColor::TextMuted
              })
              .nowrap()
              .text_overflow(TextOverflow::Elipsis)
              .width(Dimension::Px(0.0))
              .flex(1.0),
          )
          .child(icon(Icon::ChevronDown, IconSize::Md, PaletteColor::TextMuted))
      });
    Row::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .flex_shrink(0.0)
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
      .child(Element::from(select).id("instruction-targets"))
      .into()
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
