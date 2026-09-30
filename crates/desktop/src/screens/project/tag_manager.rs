use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, ScrollVertical, Select, Spacer, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::TagId, name::Name, task::organization::Tag, time::Timestamp};
use orchester_store::ProjectDb;

use super::{ProjectNav, page};
use crate::{
  background,
  messages::{self, Message},
  navigation::route::Page,
  theme::{ExtraColor, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    field::{field_error, select_style, text_input},
    form::form,
    icon::Icon,
    message_text,
  },
  workspace::OpenProject,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditingTag {
  Create,
  Rename(TagId),
  Merge(TagId),
  Delete(TagId),
}
impl lurq::app::component::DevtoolsInspectable for EditingTag {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target(Option<TagId>);
impl lurq::app::component::DevtoolsInspectable for Target {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Catalog(Vec<(Tag, usize)>);
impl lurq::app::component::DevtoolsInspectable for Catalog {}
#[derive(Clone)]
enum Mutation {
  Create(String),
  Rename(TagId, String),
  Merge(TagId, TagId),
  Delete(TagId),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Changed;
impl lurq::app::component::DevtoolsInspectable for Changed {}
type Action = FutureAction<Mutation, Changed, Message>;

pub struct TagManager {
  editing: Signal<Option<EditingTag>>,
  open: Signal<bool>,
  revision: Signal<u64>,
  name: Signal<String>,
  target: Signal<Target>,
}
impl Component for TagManager {
  type Props = OpenProject;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      editing: ctx.signal(None),
      open: ctx.signal(false),
      revision: ctx.signal(0),
      name: ctx.signal(String::new()),
      target: ctx.signal(Target(None)),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OpenProject>().clone();
    let action = self.action(ctx, &props);
    self.open.set(self.editing.get().is_some());
    let load = ctx.future((props.repo_root.clone(), self.revision.get()), |(root, _)| async move {
      background::run(move || {
        let db = ProjectDb::open(&root)?;
        db.project_tags()?
          .into_iter()
          .map(|tag| db.tag_task_count(tag.id()).map(|count| (tag, count)))
          .collect::<Result<Vec<_>, _>>()
      })
      .await?
      .map(Catalog)
      .map_err(|error| messages::store_error(&error))
    });
    let state = load.state().get();
    let pending = action.state().get().is_pending();
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    let mut body = Column::new()
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Lg)
      .flex(1.0)
      .child(
        ActionButton::new(ctx.t("tasks.tags.back"))
          .icon(Icon::ArrowLeft)
          .link(nav.link(Page::TASKS))
          .render(ctx),
      );
    if let Some(error) = state.error {
      let retry = self.revision.clone();
      body = body.child(field_error(&message_text(ctx, &error))).child(
        ActionButton::new(ctx.t("common.retry"))
          .on_click(move || retry.update(|value| *value += 1))
          .render(ctx),
      );
    } else if let Some(Catalog(tags)) = state.data {
      body = body
        .child(self.list(ctx, &tags, pending))
        .child(self.dialog(ctx, &tags, &action));
    } else {
      body = body.child(Text::new(&ctx.t("tasks.tags.loading")).variant(TypographyStyle::Caption));
    }
    Column::new()
      .height(Dimension::Pct(100.0))
      .width(Dimension::Pct(100.0))
      .child(page::topbar(
        &ctx.t_args("tasks.tags.breadcrumb", [("project", props.project.name().as_str())]),
        [ActionButton::new(ctx.t("overview.newTask"))
          .icon(Icon::Plus)
          .link(nav.link(Page::NewTask))
          .render(ctx)],
      ))
      .child(page::screen_heading(
        &ctx.t("tasks.tags.manage"),
        &ctx.t("tasks.tags.description"),
      ))
      .child(body)
  }
}

impl TagManager {
  fn action(&self, ctx: &mut Ctx, props: &OpenProject) -> Action {
    let root = props.repo_root.clone();
    let action = ctx.future_action(move |mutation: Mutation| {
      let root = root.clone();
      async move {
        background::run(move || mutate(&root, mutation)).await??;
        Ok(Changed)
      }
    });
    let (revision, editing) = (self.revision.clone(), self.editing.clone());
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      if state.data.is_some() && !state.is_pending() {
        revision.update(|value| *value += 1);
        editing.set(None);
        nav.tasks_changed();
      }
    });
    action
  }

  fn dialog(&self, ctx: &mut Ctx, tags: &[(Tag, usize)], action: &Action) -> Element {
    let Some(mode) = self.editing.get() else {
      return Modal::new(Column::new()).open_when(false).into();
    };
    let pending = action.state().get().is_pending();
    let mut content = Column::new()
      .width(Dimension::Px(MANAGER_WIDTH))
      .padding(SpacingSize::Lg)
      .spacing(SpacingSize::Lg)
      .background(ExtraColor::Surface.role())
      .rounded(RadiusSize::Lg)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(
        Text::new(&ctx.t(match mode {
          EditingTag::Create => "tasks.tags.new",
          EditingTag::Rename(_) => "tasks.tags.rename",
          EditingTag::Merge(_) => "tasks.tags.merge",
          EditingTag::Delete(_) => "tasks.tags.delete",
        }))
        .variant(ExtraTypography::DialogTitle.role()),
      )
      .child(self.editor(ctx, tags, mode, action));
    if let Some(error) = action.state().get().error {
      content = content.child(field_error(&message_text(ctx, &error)));
    }
    if pending {
      content = content.child(Text::new(&ctx.t("tasks.tags.saving")).variant(TypographyStyle::Caption));
    }
    let editing = self.editing.clone();
    ctx.watch(&self.open, move |open| {
      if !*open {
        editing.set(None);
      }
    });
    Modal::new(backdrop(ctx, form(None, content)))
      .target(ModalTarget::Root)
      .open(self.open.clone())
      .dismiss_on_escape(!pending)
      .into()
  }

  fn list(&self, ctx: &Ctx, tags: &[(Tag, usize)], pending: bool) -> Element {
    let editing = self.editing.clone();
    let name = self.name.clone();
    let mut column = Column::new().spacing(SpacingSize::Sm).child(
      ActionButton::new(ctx.t("tasks.tags.new"))
        .icon(Icon::Plus)
        .id("project-tag-new")
        .on_click(move || {
          name.set(String::new());
          editing.set(Some(EditingTag::Create));
        })
        .enabled(!pending)
        .render(ctx),
    );
    if tags.is_empty() {
      column = column.child(Text::new(&ctx.t("tasks.tags.empty")).variant(TypographyStyle::Caption));
    }
    for (tag, count) in tags {
      let id = tag.id();
      let rename = self.editing.clone();
      let name = self.name.clone();
      let current = tag.name().as_str().to_owned();
      let merge = self.editing.clone();
      let target = self.target.clone();
      let remove = self.editing.clone();
      column = column.child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(Text::new(tag.name().as_str()).variant(TypographyStyle::Body))
          .child(
            Text::new(&ctx.t_args("tasks.tags.taskCount", [("count", count.to_string().as_str())]))
              .variant(TypographyStyle::Caption),
          )
          .child(Spacer::new().flex(1.0))
          .child(
            ActionButton::new(ctx.t("tasks.tags.rename"))
              .icon(Icon::Pencil)
              .on_click(move || {
                name.set(current.clone());
                rename.set(Some(EditingTag::Rename(id)));
              })
              .enabled(!pending)
              .render(ctx)
              .id(format!("project-tag-rename-{id}")),
          )
          .child(
            ActionButton::new(ctx.t("tasks.tags.merge"))
              .icon(Icon::GitMerge)
              .on_click(move || {
                target.set(Target(None));
                merge.set(Some(EditingTag::Merge(id)));
              })
              .enabled(!pending && tags.len() > 1)
              .render(ctx)
              .id(format!("project-tag-merge-{id}")),
          )
          .child(
            ActionButton::new(ctx.t("tasks.tags.delete"))
              .icon(Icon::Trash)
              .on_click(move || remove.set(Some(EditingTag::Delete(id))))
              .enabled(!pending)
              .render(ctx)
              .id(format!("project-tag-delete-{id}")),
          ),
      );
    }
    ScrollVertical::new(column)
      .max_height(Dimension::Px(MANAGER_HEIGHT))
      .into()
  }

  fn name_input(&self, ctx: &Ctx) -> Element {
    let input = self.name.clone();
    text_input(
      ctx,
      "project-tag-name",
      input.clone(),
      self.name.get().trim().is_empty(),
      TypographyStyle::Body,
    )
    .on_input(move |event: lurq::app::events::TextInputEvent| input.set(event.new_value))
    .into()
  }

  fn editor(&self, ctx: &Ctx, tags: &[(Tag, usize)], mode: EditingTag, action: &Action) -> Element {
    let pending = action.state().get().is_pending();
    let mut column = Column::new().spacing(SpacingSize::Lg);
    let request = match mode {
      EditingTag::Create | EditingTag::Rename(_) => {
        column = column.child(self.name_input(ctx));
        if Name::new(&self.name.get()).is_err() {
          None
        } else {
          Some(match mode {
            EditingTag::Rename(id) => Mutation::Rename(id, self.name.get()),
            _ => Mutation::Create(self.name.get()),
          })
        }
      }
      EditingTag::Merge(source) => {
        let options = std::iter::once((Target(None), ctx.t("tasks.tags.chooseTarget"))).chain(
          tags
            .iter()
            .filter(|(tag, _)| tag.id() != source)
            .map(|(tag, _)| (Target(Some(tag.id())), Arc::from(tag.name().as_str()))),
        );
        column = column
          .child(
            Element::from(
              Select::new(self.target.clone())
                .options(options)
                .style(select_style(ctx, crate::tasks::organization::CONTROL_HEIGHT)),
            )
            .id("tag-merge-target"),
          )
          .child(Text::new(&ctx.t("tasks.tags.mergeExplanation")).variant(TypographyStyle::Description));
        self.target.get().0.map(|target| Mutation::Merge(source, target))
      }
      EditingTag::Delete(id) => {
        let count = tags
          .iter()
          .find(|(tag, _)| tag.id() == id)
          .map_or(0, |(_, count)| *count);
        column = column.child(
          Text::new(&ctx.t_args("tasks.tags.deleteExplanation", [("count", count.to_string().as_str())]))
            .variant(TypographyStyle::Description),
        );
        Some(Mutation::Delete(id))
      }
    };
    let cancel = self.editing.clone();
    let apply = action.clone();
    let can_confirm = request.is_some() && !pending;
    column
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .child(
            ActionButton::new(ctx.t("common.cancel"))
              .id("tag-editor-cancel")
              .in_form()
              .on_click(move || cancel.set(None))
              .enabled(!pending)
              .render(ctx),
          )
          .child(
            ActionButton::new(ctx.t("common.confirm"))
              .id("tag-editor-confirm")
              .in_form()
              .primary()
              .on_click(move || {
                if let Some(request) = &request {
                  apply.run(request.clone());
                }
              })
              .enabled(can_confirm)
              .render(ctx),
          ),
      )
      .into()
  }
}

// Project tag list y3ohgK and its confirmation panels.
const MANAGER_WIDTH: f32 = 880.0;
const MANAGER_HEIGHT: f32 = 520.0;
fn mutate(root: &std::path::Path, mutation: Mutation) -> Result<(), Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  match mutation {
    Mutation::Create(input) => db
      .create_project_tag(Name::new(&input).map_err(|error| messages::name_error(&error))?)
      .map(|_| ()),
    Mutation::Rename(id, input) => {
      db.rename_project_tag(id, &Name::new(&input).map_err(|error| messages::name_error(&error))?)
    }
    Mutation::Merge(source, target) => db.merge_project_tags(source, target, Timestamp::now()),
    Mutation::Delete(id) => db.delete_project_tag(id, Timestamp::now()),
  }
  .map_err(|error| messages::store_error(&error))
}
