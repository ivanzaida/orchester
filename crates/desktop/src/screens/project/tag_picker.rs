use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Popup, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::TagId, name::Name, task::organization::Tag};
use orchester_store::ProjectDb;

use super::ProjectNav;
use crate::{
  background,
  messages::{self, Message},
  tasks::organization::{OrganizationValue, PickerAppearance, tag_chips},
  ui::{
    button::ActionButton,
    field::{field_error, text_input},
    icon::Icon,
    message_text,
  },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TagCatalog(pub Vec<Tag>);
impl lurq::app::component::DevtoolsInspectable for TagCatalog {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct CreatedTag(Tag);
impl lurq::app::component::DevtoolsInspectable for CreatedTag {}
type CreateAction = FutureAction<String, CreatedTag, Message>;

#[derive(Clone)]
pub struct TagPickerProps {
  pub root: PathBuf,
  /// The tags being chosen; the owner keeps the same signal for the picker's lifetime.
  pub selected: Signal<OrganizationValue>,
  pub enabled: bool,
  pub appearance: PickerAppearance,
}
impl PartialEq for TagPickerProps {
  fn eq(&self, other: &Self) -> bool {
    self.root == other.root && self.enabled == other.enabled && self.appearance == other.appearance
  }
}
impl lurq::app::component::DevtoolsInspectable for TagPickerProps {}

pub struct TagPicker {
  open: Signal<bool>,
  anchor: ElementRef,
  query: Signal<String>,
  revision: Signal<u64>,
}
impl Component for TagPicker {
  type Props = TagPickerProps;
  fn create(ctx: &mut Ctx) -> Self {
    let picker = Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
      query: ctx.signal(String::new()),
      revision: ctx.signal(0),
    };
    let props = ctx.props::<TagPickerProps>().clone();
    let action = create_action(ctx, &props.root);
    picker.select_created(ctx, &action, props.selected);
    picker
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TagPickerProps>().clone();
    let action = create_action(ctx, &props.root);
    let shown = ctx
      .use_context::<super::pages::Shown>()
      .expect("shell provides project data");
    let load = ctx.future(
      (props.root.clone(), self.revision.get(), shown.revision),
      |(root, ..)| async move {
        background::run(move || ProjectDb::open(&root).and_then(|db| db.project_tags()))
          .await?
          .map(TagCatalog)
          .map_err(|error| messages::store_error(&error))
      },
    );
    let state = load.state().get();
    let tags = state.data.map(|catalog| catalog.0).unwrap_or_default();
    let selected = props.selected.get().0;
    let names: Vec<Arc<str>> = tags
      .iter()
      .filter(|tag| selected.tags().contains(&tag.id()))
      .map(|tag| Arc::from(tag.name().as_str()))
      .collect();
    let open = self.open.clone();
    let mut column = Column::new().spacing(SpacingSize::Sm).child(
      Row::new()
        .id("task-tag-chips")
        .wrap()
        .spacing(SpacingSize::Sm)
        .child(tag_chips(&names))
        .child(
          ActionButton::new(ctx.t(match (names.is_empty(), props.appearance) {
            (true, _) => "tasks.tags.add",
            (false, PickerAppearance::Header) => "tasks.tags.label",
            (false, PickerAppearance::Field) => "tasks.tags.edit",
          }))
          .icon(Icon::Tag)
          .in_form()
          .id("task-tags-picker")
          .ref_element(self.anchor.clone())
          .on_click(move || open.update(|open| *open = !*open))
          .enabled(props.enabled)
          .render(ctx),
        ),
    );
    if self.open.get() {
      let menu = self.menu(ctx, &props, &tags, &action);
      column = column.child(
        Popup::new(self.anchor.clone(), crate::ui::form::form(None, menu))
          .open(self.open.clone())
          .placement(Placement::BottomStart),
      );
    }
    if let Some(error) = state.error {
      column = column.child(field_error(&message_text(ctx, &error)));
    }
    column
  }
}

/// Creates a project tag from the typed name; the first call in a picker's `create` and every
/// `render` refer to the same action.
fn create_action(ctx: &mut Ctx, root: &std::path::Path) -> CreateAction {
  let root = root.to_path_buf();
  ctx.future_action(move |input: String| {
    let root = root.clone();
    async move {
      let name = Name::new(&input).map_err(|error| messages::name_error(&error))?;
      background::run(move || ProjectDb::open(&root).and_then(|mut db| db.create_project_tag(name)))
        .await?
        .map(CreatedTag)
        .map_err(|error| messages::store_error(&error))
    }
  })
}

impl TagPicker {
  /// Selects a tag once its creation has just succeeded, registered once per picker. A failed
  /// creation keeps the tag of the last one that succeeded as its data, which must not be
  /// selected again.
  fn select_created(&self, ctx: &mut Ctx, action: &CreateAction, selected: Signal<OrganizationValue>) {
    let (revision, query) = (self.revision.clone(), self.query.clone());
    let nav = ctx.use_context::<ProjectNav>().expect("the shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      let Some(CreatedTag(tag)) = state.data.as_ref().filter(|_| state.is_fulfilled()) else {
        return;
      };
      selected.update(|value| {
        let mut tags = value.0.tags().to_vec();
        if !tags.contains(&tag.id()) {
          tags.push(tag.id());
        }
        value.0 = value.0.clone().with_tags(tags);
      });
      query.set(String::new());
      revision.update(|revision| *revision += 1);
      nav.tasks_changed();
    });
  }

  fn menu(&self, ctx: &mut Ctx, props: &TagPickerProps, tags: &[Tag], action: &CreateAction) -> Element {
    let query = self.query.get();
    let lower = query.trim().to_lowercase();
    let input = self.query.clone();
    let selected = props.selected.get().0;
    let mut choices = Column::new().spacing(SpacingSize::Xs);
    for tag in tags
      .iter()
      .filter(|tag| tag.name().as_str().to_lowercase().contains(&lower))
    {
      let id = tag.id();
      let value = props.selected.clone();
      choices = choices.child(
        crate::ui::menu_option::menu_option(
          tag.name().as_str(),
          selected.tags().contains(&id),
          props.enabled && !action.state().get().is_pending(),
          move || toggle(&value, id),
        )
        .id(format!("picker-tag-{id}")),
      );
    }
    let pending = action.state().get().is_pending();
    let create = action.clone();
    let typed = self.query.clone();
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    let mut menu = Column::new()
      .width(Dimension::Px(MENU_WIDTH))
      .padding(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .rounded(RadiusSize::Md)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .spacing(SpacingSize::Sm)
      .child(Text::new(&ctx.t("tasks.tags.search")).variant(TypographyStyle::Caption))
      .child(
        text_input(ctx, "tag-search", input.clone(), false, TypographyStyle::Body)
          .placeholder(&ctx.t("tasks.tags.search"))
          .on_input(move |event: lurq::app::events::TextInputEvent| input.set(event.new_value)),
      )
      .child(ScrollVertical::new(choices).max_height(Dimension::Px(MENU_HEIGHT)))
      .child(
        ActionButton::new(ctx.t("tasks.tags.manage"))
          .in_form()
          .on_click(move || nav.leave(crate::navigation::route::Page::ProjectTags))
          .render(ctx),
      );
    if !lower.is_empty() && !tags.iter().any(|tag| tag.name().as_str().to_lowercase() == lower) {
      menu = menu.child(
        ActionButton::new(ctx.t_args("tasks.tags.createNamed", [("name", query.trim())]))
          .icon(Icon::Plus)
          .id("tag-create-inline")
          .in_form()
          .on_click(move || create.run(typed.get()))
          .enabled(!pending)
          .render(ctx),
      );
    }
    if tags.is_empty() {
      menu = menu.child(Text::new(&ctx.t("tasks.tags.pickerEmpty")).variant(TypographyStyle::Caption));
    } else if !lower.is_empty() && tags.iter().any(|tag| tag.name().as_str().to_lowercase() == lower) {
      menu = menu.child(Text::new(&ctx.t("tasks.tags.chooseExisting")).variant(TypographyStyle::Caption));
    }
    if pending {
      menu = menu.child(Text::new(&ctx.t("tasks.tags.saving")).variant(TypographyStyle::Caption));
    }
    if let Some(error) = action.state().get().error {
      menu = menu.child(field_error(&message_text(ctx, &error)));
    }
    menu.into()
  }
}

// IOWwP: tag options scroll inside the picker.
const MENU_HEIGHT: f32 = 240.0;
const MENU_WIDTH: f32 = 320.0;
fn toggle(selected: &Signal<OrganizationValue>, id: TagId) {
  selected.update(|value| {
    let mut tags = value.0.tags().to_vec();
    if tags.contains(&id) {
      tags.retain(|tag| *tag != id);
    } else {
      tags.push(id);
    }
    value.0 = value.0.clone().with_tags(tags);
  });
}
