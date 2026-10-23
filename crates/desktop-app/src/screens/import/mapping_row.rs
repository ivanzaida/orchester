//! One row of Resolve dependencies (`Mapping row` `ul8cc`, with a name field `Mapping row/rename` `A9wCQ`): the
//! source resource, connection, MCP server or secret and what it is; how it is resolved in this project, with the
//! name typed for a renamed copy beside the select; and what that means. A choice not available yet stays in the
//! menu, locked, with why (`uIymz`).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text, TextOverflow},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::import::{Choice, Entity};
use orchester_workspace::import::review::Planned;

use super::{ChoiceState, words::StatusLook};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ImportText},
  ui::{
    field::{FieldInput, SelectField},
    icon::{Icon, icon},
  },
};

/// The source and target columns, the select beside a name field, and the kind's glyph (`hksSJ`, `nNs2K`, `OawPj`,
/// `vO1Wf`).
pub(super) const SOURCE_WIDTH: f32 = 320.0;
pub(super) const TARGET_WIDTH: f32 = 336.0;
const RENAME_SELECT_WIDTH: f32 = 160.0;
const GLYPH: f32 = 28.0;

/// A choice of the menu, or one planned and not available yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MenuValue {
  Choice(Choice),
  Planned(Planned),
}

impl DevtoolsInspectable for MenuValue {}

/// A choice of the menu: its value, label and what it means.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Offered {
  pub choice: Choice,
  pub label: Arc<str>,
  pub detail: Option<Arc<str>>,
}

#[derive(Clone)]
pub(super) struct MappingRowProps {
  pub entity: Entity,
  pub name: Arc<str>,
  /// A secret's name is an identifier, in Geist Mono.
  pub mono: bool,
  pub glyph: Icon,
  /// What it is in the source and the content it takes.
  pub detail: Arc<str>,
  pub offered: Vec<Offered>,
  /// Planned choices, labelled, with why they are not available.
  pub planned: Vec<(Planned, Arc<str>, Arc<str>)>,
  pub choice: Choice,
  /// The name a renamed copy gets, as typed, or the source's before anything was typed.
  pub rename: String,
  pub look: StatusLook,
  pub status: Arc<str>,
  pub status_detail: Arc<str>,
  /// The page's choices, which the row changes.
  pub choices: Signal<ChoiceState>,
}

impl PartialEq for MappingRowProps {
  fn eq(&self, other: &Self) -> bool {
    self.entity == other.entity
      && self.name == other.name
      && self.mono == other.mono
      && self.glyph == other.glyph
      && self.detail == other.detail
      && self.offered == other.offered
      && self.planned == other.planned
      && self.choice == other.choice
      && self.rename == other.rename
      && self.look == other.look
      && self.status == other.status
      && self.status_detail == other.status_detail
  }
}

impl DevtoolsInspectable for MappingRowProps {}

pub(super) fn mount(ctx: &mut Ctx, props: MappingRowProps) -> Element {
  ctx.mount_keyed::<MappingRow>(&format!("import-mapping-{}", row_key(props.entity)), props)
}

struct MappingRow {
  choice: Signal<MenuValue>,
  name: Signal<String>,
}

impl Component for MappingRow {
  type Props = MappingRowProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<MappingRowProps>().clone();
    let choice = ctx.signal(MenuValue::Choice(props.choice));
    let name = ctx.signal(props.rename.clone());
    let (choices, entity) = (props.choices.clone(), props.entity);
    ctx.watch(&choice, move |chosen| {
      if let MenuValue::Choice(chosen) = *chosen {
        choices.update(|ChoiceState(choices)| choices.choose(entity, chosen));
      }
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
    Row::new()
      .id(format!("import-mapping-{key}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(source(&props, &key))
      .child(self.target(ctx, &props, &key))
      .child(resolution(&props, &key))
  }
}

impl MappingRow {
  /// The select, and beside it the name of a renamed copy (`Target` `nNs2K`, `nMBTk`).
  fn target(&self, ctx: &mut Ctx, props: &MappingRowProps, key: &str) -> Row {
    let options: Vec<SelectOption<MenuValue>> = props
      .offered
      .iter()
      .map(|offered| {
        let option = SelectOption::new(MenuValue::Choice(offered.choice), offered.label.clone());
        match &offered.detail {
          Some(detail) => option.detail(detail.clone()),
          None => option,
        }
      })
      .chain(props.planned.iter().map(|(planned, label, reason)| {
        SelectOption::new(MenuValue::Planned(*planned), label.clone())
          .detail(reason.clone())
          .disabled(true)
      }))
      .collect();
    let renaming = props.choice == Choice::Rename;
    let select = SelectField::new(
      format!("import-choice-{key}"),
      ctx.t_args("import.choice", [("name", &*props.name)]),
      self.choice.clone(),
      options,
    )
    .width(Dimension::Px(if renaming { RENAME_SELECT_WIDTH } else { TARGET_WIDTH }));
    // A row without a name field shows its problem on the select: a choice that is gone, or a name taken.
    let select = match (props.look.problem, renaming, &props.look.tint) {
      (true, false, PaletteColor::Warning) => select.warning(),
      (true, false, _) => select.invalid(),
      _ => select,
    };
    let mut target = Row::new()
      .width(Dimension::Px(TARGET_WIDTH))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(select.render(ctx));
    if renaming {
      let field = FieldInput::new(
        format!("import-rename-{key}"),
        ctx.t_args("import.renameName", [("name", &*props.name)]),
        self.name.clone(),
      )
      .placeholder(ctx.t("import.renamePlaceholder"));
      let field = if props.look.problem { field.invalid() } else { field };
      target = target.child(
        Row::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(field.render(ctx)),
      );
    }
    target
  }
}

/// The kind's glyph, the name and what it is (`Source` `hksSJ`).
fn source(props: &MappingRowProps, key: &str) -> Row {
  let name = Text::new(&props.name).id(format!("import-source-{key}")).nowrap();
  let name = if props.mono {
    name.variant(ImportText::SecretName.role())
  } else {
    name.variant(ExtraTypography::Strong.role())
  };
  Row::new()
    .width(Dimension::Px(SOURCE_WIDTH))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .width(Dimension::Px(GLYPH))
        .height(Dimension::Px(GLYPH))
        .flex_shrink(0.0)
        .align_items(Alignment::Center)
        .justify(Justify::Center)
        .rounded(RadiusSize::Md)
        .background(PaletteColor::SurfaceRaised)
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(icon(props.glyph, IconSize::Md, PaletteColor::TextSecondary)),
    )
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(name.text_overflow(TextOverflow::Elipsis))
        .child(
          Text::new(&props.detail)
            .id(format!("import-detail-{key}"))
            .variant(ExtraTypography::Note.role())
            .nowrap(),
        ),
    )
}

/// How it is resolved and what that means (`Resolution` `A0OHX`).
fn resolution(props: &MappingRowProps, key: &str) -> Column {
  Column::new()
    .id(format!("import-status-{key}"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .describe("problem", if props.look.problem { "true" } else { "false" })
    .child(
      Row::new()
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(props.look.glyph, IconSize::Base, props.look.tint.clone()))
        .child(
          Text::new(&props.status)
            .id(format!("import-status-{key}-label"))
            .variant(TypographyStyle::Label)
            .color(props.look.word.clone())
            .nowrap(),
        ),
    )
    .child(
      Text::new(&props.status_detail)
        .id(format!("import-status-{key}-detail"))
        .width(Dimension::Pct(100.0))
        .variant(ExtraTypography::Note.role()),
    )
}

/// The part of element ids that names the row's source entity.
pub(super) fn row_key(entity: Entity) -> String {
  match entity {
    Entity::Resource(id) => id.to_string(),
    Entity::Connection(id) | Entity::McpServer(id) => id.to_string(),
    Entity::Secret(id) => id.to_string(),
  }
}
