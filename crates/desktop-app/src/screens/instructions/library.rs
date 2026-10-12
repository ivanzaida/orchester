//! Virtualized scope library b1i8f, using the shared rule item zmP3o.
use super::{
  state::{Selection, Tab, View},
  words,
};
use crate::{
  theme::{ExtraSpacing, instructions},
  ui::rule_item::RuleItem,
};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Text, VirtualizedList},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::InstructionScope;
use orchester_workspace::instructions::data::{InstructionEntry, ProjectInstructions};
use std::sync::Arc;
// Rule list b1i8f.
const LIST_WIDTH: f32 = 248.0;

#[derive(Clone, PartialEq)]
enum Item {
  Group(String),
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
      Item::Group(group) => {
        return Element::from(
          Text::new(&group)
            .variant(TypographyStyle::FieldLabel)
            .padding_top(SpacingSize::Sm),
        );
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
    Element::from(RuleItem::new(
      format!("instruction-{id}"),
      entry.name,
      words::version(ctx, entry.state),
      meta,
      props.edited == Some(id),
      move || props.selection.open(View::Rule(props.tab, Some(id))),
    ))
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
  let mut items = Vec::new();
  let entries = data
    .entries
    .iter()
    .filter(|e| Tab::of(e.body.scope()) == tab)
    .map(|entry| edited.filter(|e| e.id == entry.id).unwrap_or(entry).clone())
    .collect::<Vec<_>>();
  let mut groups = std::collections::BTreeMap::<String, Vec<InstructionEntry>>::new();
  for entry in entries {
    let group = match entry.body.scope() {
      InstructionScope::AgentRole { role } => role.as_str().to_owned(),
      InstructionScope::Pipeline { pipeline } | InstructionScope::Step { pipeline, .. } => data
        .pipeline(*pipeline)
        .map_or_else(|| pipeline.to_string(), |p| p.name.to_string()),
      InstructionScope::Project => String::new(),
    };
    groups.entry(group).or_default().push(entry);
  }
  if tab == Tab::Roles {
    for role in data.roles() {
      groups.entry(role.as_str().to_owned()).or_default();
    }
  } else if matches!(tab, Tab::Pipelines | Tab::Steps) {
    for pipeline in data.pipelines.iter() {
      groups.entry(pipeline.name.to_string()).or_default();
    }
  }
  for (group, entries) in groups {
    if !group.is_empty() {
      items.push(Item::Group(
        ctx
          .t_args(
            "instructions.groupCount",
            [("name", group), ("count", entries.len().to_string())],
          )
          .to_string(),
      ));
    }
    items.extend(entries.into_iter().map(Item::Rule));
  }
  let (listed, selection, selected) = (data.clone(), selection.clone(), edited.map(|e| e.id));
  let rows = VirtualizedList::new_shared(ctx, Arc::new(items))
    .list_key("instruction-library-rows")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .mount_keyed::<LibraryItem, _, _, _>(
      |item| match item {
        Item::Group(group) => group.clone(),
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
    .child(Text::new(&ctx.t(tab.key()).to_uppercase()).variant(instructions::overline()))
    .child(rows)
    .child(Text::new(&ctx.t_args("instructions.ownership", [("project", project)])).variant(TypographyStyle::Caption))
    .into()
}
