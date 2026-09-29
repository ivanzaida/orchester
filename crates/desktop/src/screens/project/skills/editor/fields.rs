//! The skill's column (`Yr80m` in `ecJtg`): its name and state, its name and scope, the description
//! an agent reads to decide whether to load it, its body (the `SKILL.md`) and what publishing
//! changes.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Select, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::resource::{InstructionScope, SKILL_FILE, Skill, SkillError};

use super::{super::copy, SkillEditor, skill_error};
use crate::{
  agents::data::VersionState,
  instructions::form::Picked,
  screens::project::instructions::copy::draft_number,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    field::{field_error, mono_value_box, select_style, styled_input, text_input},
    icon::{Icon, IconSize, icon},
    message_text,
    status_label::status_label,
  },
};

/// Field height (`un4ZX`, `K6d9b`, `EKBFh`) and the rows the body grows between (`YueT2`).
const FIELD_HEIGHT: f32 = 36.0;
const BODY_ROWS: (usize, usize) = (18, 400);

impl SkillEditor {
  pub(super) fn skill_editor(
    &self,
    ctx: &Ctx,
    live: &Skill,
    state: VersionState,
    error: Option<&SkillError>,
  ) -> Element {
    let (glyph, key) = if state.has_draft() {
      (Icon::FilePenLine, "skills.state.draft")
    } else {
      (Icon::Check, "skills.state.published")
    };
    let version = if state.has_draft() {
      draft_number(state)
    } else {
      state.published().map_or(1, |version| version.get())
    };
    let label = ctx.t_args(
      key,
      [
        ("kind", &*copy::kind(ctx, live.scope())),
        ("version", version.to_string().as_str()),
      ],
    );
    let header = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .child(
        Text::new(&self.context.entry.name)
          .variant(ExtraTypography::VariableName.role())
          .id("skill-name"),
      )
      .child(Row::new().child(status_label(glyph, &label)));
    let settings = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(setting(
        ctx,
        "skills.editor.name",
        "skill-name-field",
        mono_value_box(
          ctx,
          "skill-name-value",
          &self.context.entry.name,
          Dimension::Px(FIELD_HEIGHT),
        ),
      ))
      .child(setting(
        ctx,
        "skills.editor.scope",
        "skill-scope",
        self.scope_select(ctx),
      ));
    let description = text_input(
      ctx,
      "skill-description",
      self.description.clone(),
      false,
      TypographyStyle::Caption,
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t("skills.editor.descriptionPlaceholder"));
    Column::new()
      .id("skill-editor")
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .spacing(ExtraSpacing::Step.role())
      .child(header)
      .child(settings)
      .child(
        Column::new()
          .id("skill-description-field")
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Field.role())
          .child(Text::new(&ctx.t("skills.editor.description")).variant(TypographyStyle::Caption))
          .child(description),
      )
      .child(self.body_editor(ctx, error))
      .child(footer(ctx, state))
      .into()
  }

  /// One select for where the skill applies: the project, a role, a pipeline or a step (`K6d9b`).
  fn scope_select(&self, ctx: &Ctx) -> Element {
    let data = &self.context.data;
    let mut scopes = vec![InstructionScope::Project];
    scopes.extend(
      data
        .roles()
        .into_iter()
        .map(|role| InstructionScope::AgentRole { role }),
    );
    scopes.extend(
      data
        .pipelines
        .iter()
        .map(|pipeline| InstructionScope::Pipeline { pipeline: pipeline.id }),
    );
    scopes.extend(data.pipelines.iter().flat_map(|pipeline| {
      pipeline.steps.iter().map(|step| InstructionScope::Step {
        pipeline: pipeline.id,
        step: step.key.clone(),
      })
    }));
    // A scope whose role, pipeline or step is gone stays chosen until another is.
    let current = self.scope.get().0;
    if !scopes.contains(&current) {
      scopes.push(current);
    }
    let options: Vec<(Picked<InstructionScope>, Arc<str>)> = scopes
      .into_iter()
      .map(|scope| {
        let label = copy::scope(ctx, &scope, data, true);
        (Picked(scope), label)
      })
      .collect();
    Select::new(self.scope.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FIELD_HEIGHT))
      .style(select_style(ctx, FIELD_HEIGHT))
      .into()
  }

  /// "Body" with its format, and the body (`yqyiC`, `dUWfC`).
  fn body_editor(&self, ctx: &Ctx, error: Option<&SkillError>) -> Element {
    let toolbar = Row::new()
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .justify(Justify::SpaceBetween)
      .child(Text::new(&ctx.t("skills.editor.body")).variant(ExtraTypography::FormLabel.role()))
      .child(Text::new(&ctx.t_args("skills.editor.bodyFormat", [("file", SKILL_FILE)])).variant(TypographyStyle::Mono));
    let input = styled_input(ctx, self.body.clone(), ExtraTypography::PromptText.role())
      .id("skill-body")
      .textarea()
      .rows(BODY_ROWS.0, BODY_ROWS.1)
      .width(Dimension::Pct(100.0))
      .placeholder(&ctx.t("skills.editor.bodyPlaceholder"));
    let area = ScrollVertical::new(
      // The design's first line starts 18 below the edge plus the half-leading of its 1.8 lines,
      // which lurq's trimmed line box leaves out, as in the instruction editor.
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(ExtraSpacing::Divider.role())
        .padding_top(SpacingSize::Lg)
        .child(input),
    )
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Notice.role());
    Column::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .spacing(ExtraSpacing::Step.role())
      .child(toolbar)
      .child(area)
      .with_children(error.map(|error| field_error(&message_text(ctx, &skill_error(error)))))
      .into()
  }
}

/// A setting beside another (`SeNhn`, `H6VO7G`): its label over its control, sharing the row.
fn setting(ctx: &Ctx, label: &str, id: &'static str, control: Element) -> Element {
  Column::new()
    .id(id)
    .flex(1.0)
    .spacing(ExtraSpacing::Field.role())
    .child(Text::new(&ctx.t(label)).variant(TypographyStyle::Caption))
    .child(control)
    .into()
}

/// What publishing changes (`DAzfk`).
fn footer(ctx: &Ctx, state: VersionState) -> Element {
  let text = match state.published() {
    Some(version) => ctx.t_args(
      "skills.editor.publishEffect",
      [("version", version.get().to_string().as_str())],
    ),
    None => ctx.t("skills.editor.publishEffectFirst"),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::History, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(ExtraTypography::Footnote.role()))
    .into()
}
