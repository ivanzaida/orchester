//! Virtualized scope library b1i8f, using the shared rule item zmP3o.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, VirtualizedList},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::InstructionScope;
use orchester_workspace::{
  agents::data::VersionState,
  instructions::data::{InstructionEntry, ProjectInstructions},
};

use super::{
  state::{Selection, Tab, View},
  words,
};
use crate::{
  theme::{ExtraSpacing, IconSize, instructions},
  ui::{
    icon::{Icon, icon},
    rule_item::RuleItem,
  },
};
// Rule list b1i8f.
const LIST_WIDTH: f32 = 248.0;

#[derive(Clone, PartialEq)]
pub(super) enum Item {
  Group { name: String, count: String, role: bool },
  Step { name: String, role: Option<String> },
  Ownership(String),
  Rule(InstructionEntry),
}
#[derive(Clone)]
struct Props {
  item: Item,
  data: ProjectInstructions,
  tab: Tab,
  edited: Option<orchester_core::id::ResourceId>,
  selection: Selection,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.item == other.item && self.data == other.data && self.tab == other.tab && self.edited == other.edited
  }
}
impl DevtoolsInspectable for Props {}
struct LibraryItem;
impl Component for LibraryItem {
  type Props = Props;
  fn create(_: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let entry = match props.item {
      Item::Group { name, count, role } => {
        return Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .padding(Padding::new().top(SpacingSize::Sm).bottom(SpacingSize::Xs))
          .child(icon(
            if role { Icon::Bot } else { Icon::Workflow },
            IconSize::Md,
            PaletteColor::TextMuted,
          ))
          .child(Text::new(&name).variant(TypographyStyle::Label).flex(1.0))
          .child(Text::new(&count).variant(TypographyStyle::Caption))
          .into();
      }
      Item::Step { name, role } => {
        return Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
          .child(icon(Icon::CornerDownRight, IconSize::Md, PaletteColor::TextMuted))
          .child(Text::new(&name).variant(TypographyStyle::Description).flex(1.0))
          .with_children(role.map(|role| Text::new(&role).variant(TypographyStyle::Caption)))
          .into();
      }
      Item::Ownership(project) => {
        return Text::new(&ctx.t_args("instructions.ownership", [("project", project)]))
          .variant(TypographyStyle::Caption)
          .padding_top(SpacingSize::Md)
          .into();
      }
      Item::Rule(entry) => entry,
    };
    let id = entry.id;
    let meta = ctx.t_args(
      "instructions.rowMeta",
      [
        ("targets", words::targets(ctx, entry.body.targets(), &props.data)),
        (
          "event",
          words::event(
            ctx,
            orchester_workspace::instructions::form::Events::of(entry.body.events()),
          ),
        ),
      ],
    );
    let meta = if matches!(
      entry.body.scope(),
      InstructionScope::AgentRole { .. } | InstructionScope::Step { .. }
    ) {
      words::scoped_event(
        ctx,
        orchester_workspace::instructions::form::Events::of(entry.body.events()),
        matches!(entry.body.scope(), InstructionScope::AgentRole { .. }),
      )
    } else {
      meta
    };
    let published = matches!(entry.state, VersionState::Published(_));
    let row = RuleItem::new(
      format!("instruction-{id}"),
      entry.name,
      words::version(ctx, entry.state),
      meta,
      props.edited == Some(id),
      move || props.selection.open(View::Rule(props.tab, Some(id))),
    );
    Element::from(if published {
      row.published()
    } else if entry.state.published().is_none() {
      row.first_draft()
    } else {
      row
    })
  }
}
pub fn list(
  ctx: &mut Ctx,
  data: &ProjectInstructions,
  tab: Tab,
  edited: Option<&InstructionEntry>,
  project: &str,
  selection: &Selection,
) -> Element {
  let entries = data
    .entries
    .iter()
    .filter(|e| Tab::of(e.body.scope()) == tab)
    .map(|entry| edited.filter(|e| e.id == entry.id).unwrap_or(entry).clone())
    .collect::<Vec<_>>();
  let mut items = super::library_groups::items(ctx, data, tab, entries);
  items.push(Item::Ownership(project.to_owned()));
  let (listed, selection, selected) = (data.clone(), selection.clone(), edited.map(|e| e.id));
  let rows = VirtualizedList::new_shared(ctx, Arc::new(items))
    .list_key("instruction-library-rows")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .mount_keyed::<LibraryItem, _, _, _>(
      |item| match item {
        Item::Group { name, .. } => name.clone(),
        Item::Step { name, .. } => name.clone(),
        Item::Ownership(_) => "ownership".to_owned(),
        Item::Rule(e) => e.id.to_string(),
      },
      move |item| Props {
        item: item.clone(),
        data: listed.clone(),
        tab,
        edited: selected,
        selection: selection.clone(),
      },
    );
  Column::new()
    .id("instruction-library")
    .width(Dimension::Px(LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Md))
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(
        &ctx
          .t(if tab == Tab::Roles {
            "instructions.agentRoleRules"
          } else {
            tab.key()
          })
          .to_uppercase(),
      )
      .variant(instructions::overline()),
    )
    .child(rows)
    .into()
}
