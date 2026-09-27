//! One row of the dependency mapping (`HfNoA`): the source resource, connection or secret and the
//! content it takes; how it is resolved in this project, with the name typed for a renamed copy;
//! and what that means.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, SelectStyle, border::Border, dimension::Dimension},
};
use orchester_core::import::{Choice, Entity};

use super::ChoiceState;
use crate::{
  theme::{ExtraColor, ExtraTypography},
  ui::field::{select_style, select_trigger, text_input},
};

/// Row, select and column sizes (`HfNoA`, `u8vYa`, `a2i3d`, `galyn`).
const ROW_HEIGHT: f32 = 56.0;
const SELECT_HEIGHT: f32 = 34.0;
pub(super) const SOURCE_WIDTH: f32 = 240.0;
pub(super) const RESOLUTION_WIDTH: f32 = 196.0;

/// A choice as a menu value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MappingChoice(pub Choice);

impl lurq::app::component::DevtoolsInspectable for MappingChoice {}

#[derive(Clone)]
pub(super) struct MappingRowProps {
  pub entity: Entity,
  pub name: Arc<str>,
  /// What it is in the source and which content a copy takes.
  pub detail: Arc<str>,
  pub options: Vec<(MappingChoice, Arc<str>)>,
  pub choice: Choice,
  /// The name a renamed copy gets, as typed.
  pub rename: String,
  pub status: Arc<str>,
  pub status_detail: Arc<str>,
  /// Something keeps the import from starting until this row changes.
  pub problem: bool,
  /// The dialog's choices, which the row changes.
  pub choices: Signal<ChoiceState>,
}

impl PartialEq for MappingRowProps {
  fn eq(&self, other: &Self) -> bool {
    self.entity == other.entity
      && self.name == other.name
      && self.detail == other.detail
      && self.options == other.options
      && self.choice == other.choice
      && self.rename == other.rename
      && self.status == other.status
      && self.status_detail == other.status_detail
      && self.problem == other.problem
  }
}

impl lurq::app::component::DevtoolsInspectable for MappingRowProps {}

pub(super) struct MappingRow {
  choice: Signal<MappingChoice>,
  name: Signal<String>,
}

impl Component for MappingRow {
  type Props = MappingRowProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<MappingRowProps>().clone();
    let choice = ctx.signal(MappingChoice(props.choice));
    let name = ctx.signal(props.rename.clone());
    let (choices, entity) = (props.choices.clone(), props.entity);
    ctx.watch(&choice, move |MappingChoice(chosen)| {
      let chosen = *chosen;
      choices.update(|ChoiceState(choices)| choices.choose(entity, chosen));
    });
    let choices = props.choices;
    ctx.watch(&name, move |typed| {
      let typed = typed.clone();
      choices.update(|ChoiceState(choices)| choices.rename(entity, typed));
    });
    Self { choice, name }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MappingRowProps>().clone();
    let key = row_key(props.entity);
    let mut target = Row::new()
      .flex(1.0)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Column::new().flex(1.0).child(
          Element::from(
            Select::new(self.choice.clone())
              .options(props.options.clone())
              .width(Dimension::Pct(100.0))
              .height(Dimension::Px(SELECT_HEIGHT))
              .style(mapping_style(ctx)),
          )
          .id(format!("import-choice-{key}")),
        ),
      );
    if self.choice.get().0 == Choice::Rename {
      target = target.child(
        text_input(
          ctx,
          "import-rename",
          self.name.clone(),
          props.problem,
          TypographyStyle::FieldLabel,
        )
        .id(format!("import-rename-{key}"))
        .height(Dimension::Px(SELECT_HEIGHT))
        .flex(1.0)
        .placeholder(&ctx.t("projectImport.renamePlaceholder")),
      );
    }
    Row::new()
      .id(format!("import-mapping-{key}"))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(cell(
        SOURCE_WIDTH,
        &props.name,
        TypographyStyle::FieldLabel,
        &props.detail,
      ))
      .child(target)
      .child(
        cell(
          RESOLUTION_WIDTH,
          &props.status,
          TypographyStyle::Caption,
          &props.status_detail,
        )
        .id(format!("import-status-{key}")),
      )
  }
}

/// A select drawn like a field, its value in the smaller text of the mapping (`ObbY7` in `u8vYa`).
fn mapping_style(ctx: &Ctx) -> SelectStyle {
  let trigger = select_trigger(ctx, SELECT_HEIGHT)
    .typography(TypographyStyle::Caption)
    .text_color(PaletteColor::TextPrimary);
  select_style(ctx, SELECT_HEIGHT).trigger(trigger)
}

/// A fixed-width cell of a line over a smaller line (`a2i3d`, `galyn`).
fn cell(width: f32, title: &str, typography: TypographyStyle, detail: &str) -> Column {
  Column::new()
    .width(Dimension::Px(width))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(title)
        .variant(typography)
        .color(PaletteColor::TextPrimary)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(
      Text::new(detail)
        .variant(ExtraTypography::Footnote.role())
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}

/// The part of element ids that names the row's source entity.
pub(super) fn row_key(entity: Entity) -> String {
  match entity {
    Entity::Resource(id) => id.to_string(),
    Entity::Connection(id) => id.to_string(),
    Entity::Secret(id) => id.to_string(),
  }
}
