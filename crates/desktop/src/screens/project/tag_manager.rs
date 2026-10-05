mod editor;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Modal, ModalTarget, Row, ScrollVertical, Select, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{id::TagId, name::Name, task::organization::Tag};
use orchester_store::ProjectDb;

use super::{ProjectNav, page};
use crate::{
  background,
  messages::{self, Message},
  navigation::route::Page,
  tasks::writer::TaskWriter,
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
    let manager = Self {
      editing: ctx.signal(None),
      open: ctx.signal(false),
      revision: ctx.signal(0),
      name: ctx.signal(String::new()),
      target: ctx.signal(Target(None)),
    };
    let root = ctx.props::<OpenProject>().repo_root.clone();
    let action = action(ctx, &root);
    manager.watch(ctx, &action);
    manager
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OpenProject>().clone();
    let action = action(ctx, &props.repo_root);
    self.open.set(self.editing.get().is_some());
    let load = ctx.future((props.repo_root.clone(), self.revision.get()), |(root, _)| async move {
      background::run(move || {
        let db = ProjectDb::open(&root)?;
        let counts = db.tag_task_counts()?;
        // A tag on no task has no count of its own.
        let tags = db.project_tags()?.into_iter().map(|tag| {
          let count = counts.get(&tag.id()).copied().unwrap_or_default();
          (tag, count)
        });
        Ok::<_, orchester_store::StoreError>(tags.collect::<Vec<_>>())
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
      .flex(1.0);
    if let Some(error) = state.error {
      let retry = self.revision.clone();
      body = body.child(field_error(&message_text(ctx, &error))).child(
        ActionButton::new(ctx.t("common.retry"))
          .on_click(move || retry.update(|value| *value += 1))
          .render(ctx),
      );
    } else if let Some(Catalog(tags)) = state.data {
      body = if matches!(
        self.editing.get(),
        Some(EditingTag::Create | EditingTag::Rename(_) | EditingTag::Merge(_))
      ) {
        body.child(self.dialog(ctx, &tags, &action))
      } else {
        body
          .child(self.list(ctx, &tags, pending))
          .child(self.dialog(ctx, &tags, &action))
      };
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
      .child(page::organization_heading(
        &ctx.t("tasks.tags.projectTitle"),
        &ctx.t_args("tasks.tags.projectHint", [("project", props.project.name().as_str())]),
      ))
      .child(body)
  }
}

/// Applies a change to the project's tags; the call in the manager's `create` and every `render`
/// refer to the same action.
fn action(ctx: &mut Ctx, root: &std::path::Path) -> Action {
  let root = root.to_path_buf();
  let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
  ctx.future_action(move |mutation: Mutation| {
    let (root, servers) = (root.clone(), servers.clone());
    async move {
      background::run(move || mutate(&root, &servers, mutation)).await??;
      Ok(Changed)
    }
  })
}

impl TagManager {
  /// Registered once: a change that has just succeeded closes its editor and rereads the tags. A
  /// failed change keeps the data of the last one that succeeded, so it leaves the editor open
  /// with its error. Closing the delete confirmation's modal ends the delete.
  fn watch(&self, ctx: &mut Ctx, action: &Action) {
    let (revision, editing) = (self.revision.clone(), self.editing.clone());
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() {
        revision.update(|value| *value += 1);
        editing.set(None);
        nav.tasks_changed();
      }
    });
    let editing = self.editing.clone();
    ctx.watch(&self.open, move |open| {
      if !*open {
        editing.set(None);
      }
    });
  }

  fn list_actions(&self, ctx: &mut Ctx, pending: bool) -> Element {
    let nav = ctx.use_context::<ProjectNav>().expect("shell provides navigation");
    let (name, editing) = (self.name.clone(), self.editing.clone());
    Row::new()
      .spacing(SpacingSize::Sm)
      .child(
        ActionButton::new(ctx.t("tasks.tags.back"))
          .icon(Icon::ArrowLeft)
          .link(nav.link(Page::TASKS))
          .render(ctx),
      )
      .child(
        ActionButton::new(ctx.t("tasks.tags.new"))
          .icon(Icon::Plus)
          .id("project-tag-new")
          .on_click(move || {
            name.set(String::new());
            editing.set(Some(EditingTag::Create));
          })
          .enabled(!pending)
          .render(ctx),
      )
      .into()
  }

  /// One project tag (`y3ohgK`): its chip, how many tasks use it, then Rename, Merge and Delete.
  fn tag_row(&self, ctx: &mut Ctx, (tag, count): (&Tag, usize), pending: bool, can_merge: bool) -> Element {
    let id = tag.id();
    let rename = self.editing.clone();
    let name = self.name.clone();
    let current = tag.name().as_str().to_owned();
    let merge = self.editing.clone();
    let target = self.target.clone();
    let remove = self.editing.clone();
    Row::new()
      .height(Dimension::Px(TAG_ROW_HEIGHT))
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .spacing(crate::theme::ExtraSpacing::Stack.role())
      .child(
        Column::new()
          .flex(1.0)
          .child(crate::tasks::organization::tag_chip(tag.name().as_str())),
      )
      .child(
        Row::new().width(Dimension::Px(TAG_COUNT_WIDTH)).child(
          Text::new(&if count == 1 {
            ctx.t("tasks.tags.taskCountOne")
          } else {
            ctx.t_args("tasks.tags.taskCount", [("count", count.to_string().as_str())])
          })
          .variant(TypographyStyle::Description),
        ),
      )
      .child(
        Row::new()
          .width(Dimension::Px(TAG_ACTIONS_WIDTH))
          .spacing(crate::theme::ExtraSpacing::Stack.role())
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
              .enabled(!pending && can_merge)
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
      )
      .into()
  }

  fn list(&self, ctx: &mut Ctx, tags: &[(Tag, usize)], pending: bool) -> Element {
    let actions = self.list_actions(ctx, pending);
    let mut column = Column::new().padding_top(SpacingSize::Lg);
    if tags.is_empty() {
      column = column.child(Text::new(&ctx.t("tasks.tags.empty")).variant(TypographyStyle::Caption));
    }
    for (tag, count) in tags {
      column = column.child(self.tag_row(ctx, (tag, *count), pending, tags.len() > 1));
    }
    if !tags.is_empty() {
      column = column.child(
        Text::new(&ctx.t("tasks.tags.styleNote"))
          .variant(TypographyStyle::Caption)
          .padding_top(SpacingSize::Lg),
      );
    }
    ScrollVertical::new(Column::new().child(actions).child(column))
      .max_height(Dimension::Px(MANAGER_HEIGHT))
      .into()
  }
}

// Project tag list y3ohgK and its confirmation panels.
const TAG_ROW_HEIGHT: f32 = 64.0;
const TAG_COUNT_WIDTH: f32 = 120.0;
const TAG_ACTIONS_WIDTH: f32 = 340.0;
const MANAGER_HEIGHT: f32 = 520.0;
fn mutate(root: &std::path::Path, servers: &crate::sharing::Servers, mutation: Mutation) -> Result<(), Message> {
  let mut writer = TaskWriter::open(root, servers).map_err(|error| error.message())?;
  match mutation {
    Mutation::Create(input) => writer
      .create_tag(Name::new(&input).map_err(|error| messages::name_error(&error))?)
      .map(|_| ()),
    Mutation::Rename(id, input) => {
      writer.rename_tag(id, &Name::new(&input).map_err(|error| messages::name_error(&error))?)
    }
    Mutation::Merge(source, target) => writer.merge_tags(source, target),
    Mutation::Delete(id) => writer.delete_tag(id),
  }
  .map_err(|error| error.message())
}
