//! Tools, the Agents hub's fourth tab (`04 Tools` `c05kx`): the project's tools, and a tool's page with its
//! Configuration, Inputs & outputs, Access and Usage log tabs (`04b`–`04e`).
mod access;
mod add_menu;
mod editor;
mod editor_asks;
mod fields;
mod fields_rows;
mod head;
mod inputs;
mod inspector;
mod library;
mod validation;
mod words;
mod work;

use editor::{EditorProps, ToolEditor};
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ResourceId;
use orchester_workspace::{background, messages, tasks::outside::Outside, tools::editor::EditorData};

use crate::{
  project::{ShownProject, own::OwnChanges},
  routes::ProjectPage,
  screens::hub::{self, Head, HubTab},
  theme::HubSpacing,
  ui::failure::LastFailure,
};

#[derive(Clone, PartialEq)]
pub struct Props {
  project: ShownProject,
  page: ProjectPage,
}
impl DevtoolsInspectable for Props {}
struct ToolsScreen {
  /// Counts the reads a save or Reload asks for; read retries keep the editor's mount key.
  revision: Signal<u64>,
  /// The revision Reload started the page over at.
  reset_at: Signal<u64>,
  retry: Signal<u64>,
  /// The tool changes, the screen's own too.
  tools: OwnChanges,
  /// Add tool's menu in the head.
  menu: Signal<bool>,
  anchor: ElementRef,
  /// Why the tools couldn't be read, kept while they are read again.
  failure: LastFailure,
}
impl Component for ToolsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
      reset_at: ctx.signal(0),
      retry: ctx.signal(0),
      tools: OwnChanges::new(ctx, &[Outside::Tools]),
      menu: ctx.signal(false),
      anchor: ctx.element_ref(),
      failure: LastFailure::default(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let body = self.body(ctx);
    // A member of a shared project adds none of the team's tools.
    let head = if hub::team::may_change(hub::team::of(&project).as_ref()) {
      Head::actions(add_menu::actions(
        ctx,
        project.open.project.id(),
        (&self.menu, &self.anchor),
      ))
    } else {
      Head::default()
    };
    Column::new()
      .id("screen-tools")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(hub::page(ctx, &project, (HubTab::Tools, head), body))
  }
}
impl ToolsScreen {
  fn body(&self, ctx: &mut Ctx) -> Element {
    let props = ctx.props::<Props>().clone();
    let id = match props.page {
      ProjectPage::Tool(id, _) => Some(id),
      _ => None,
    };
    let root = props.project.open.repo_root.clone();
    let revision = self.revision.get();
    let outside = props.project.feed.outside.get();
    let outside = (outside.pipelines, self.tools.count(&props.project.feed));
    let hold = crate::services::Services::of(ctx).hold;
    let reading = ctx
      .future(
        (root, id, revision, outside, self.retry.get()),
        move |(root, id, revision, ..)| {
          tracing::trace!(target: crate::app_log::READ_LOG, read = "tools");
          let hold = hold.clone();
          async move {
            hold.released().await;
            background::run(move || EditorData::load_in(&root, id))
              .await?
              .map(|data| (id, revision, data))
              .map_err(|error| messages::store_error(&error))
          }
        },
      )
      .state()
      .get();
    let frame = Column::new().width(Dimension::Pct(100.0)).height(Dimension::Pct(100.0));
    let retry = self.retry.clone();
    let what = ctx.t("tools.loadFailed");
    let failure = hub::read_failure(
      ctx,
      &props.project,
      (HubTab::Tools, "tools-load-error", &what),
      self.failure.follow(&reading),
      move || retry.update(|r| *r += 1),
    );
    // A tool's page keeps showing while its own save is read again; after Reload it waits for the new read.
    let reset = self.reset_at.get();
    let current = reading
      .data
      .filter(|(read_id, read_revision, _)| *read_id == id && *read_revision >= reset);
    let (failure, data) = match (failure, current) {
      (Some(failure), current) if current.is_none() || matches!(props.page, ProjectPage::Tools) => {
        return hub::list_body("tools-failed", failure.page(ctx));
      }
      (_, None) => return frame.child(library::reading(ctx, &props.project)).into(),
      (failure, Some((_, _, data))) => (failure, data),
    };
    let child = self.content(ctx, props, data, id);
    frame
      .with_children(failure.map(|failure| {
        // What was read stays under it.
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(
            Padding::new()
              .left(HubSpacing::Side.role())
              .right(HubSpacing::Side.role())
              .top(HubSpacing::BodyTop.role()),
          )
          .child(failure.page(ctx))
      }))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(child),
      )
      .into()
  }
  fn content(&self, ctx: &mut Ctx, props: Props, data: EditorData, id: Option<ResourceId>) -> Element {
    match props.page {
      ProjectPage::Tools => library::list(ctx, &props.project, &data.listed),
      ProjectPage::NewTool | ProjectPage::Tool(..) => {
        if id.is_some_and(|id| data.listed.tool(id).is_none()) {
          return crate::ui::notice::Notice::new(
            "tool-missing",
            crate::ui::notice::NoticeTone::Danger,
            ctx.t("tools.notFound"),
          )
          .into();
        }
        // A save keeps the page; Reload and Discard draft start it over.
        let reset = self.reset_at.get();
        let entry = id.and_then(|id| data.listed.tool(id)).cloned();
        let key = format!(
          "tool-editor-{}-{reset}",
          id.map_or_else(|| "new".to_owned(), |id| id.to_string())
        );
        ctx.mount_keyed::<ToolEditor>(
          &key,
          EditorProps {
            project: props.project,
            data,
            entry,
            revision: self.revision.clone(),
            reload: editor::Reload {
              revision: self.revision.clone(),
              reset_at: self.reset_at.clone(),
            },
            tools: self.tools.clone(),
            read_revision: self.retry.get(),
            page: props.page,
          },
        )
      }
      _ => unreachable!("Tools mounts only its own pages"),
    }
  }
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, page: &ProjectPage) -> Element {
  ctx.mount_keyed::<ToolsScreen>(
    &format!("tools-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      page: page.clone(),
    },
  )
}
