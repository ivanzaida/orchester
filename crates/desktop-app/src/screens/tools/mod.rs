//! Project command tools, layer Tools `m8Aec`.
mod access;
mod editor;
mod fields;
mod head;
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
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::outside::Outside,
  tools::editor::EditorData,
};

use crate::{
  project::{ShownProject, own::OwnChanges},
  routes::ProjectPage,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
pub struct Props {
  project: ShownProject,
  page: ProjectPage,
}
impl DevtoolsInspectable for Props {}
struct ToolsScreen {
  // Save and explicit Reload reset fields; read retries keep the editor's mount key.
  revision: Signal<u64>,
  retry: Signal<u64>,
  /// The tool changes, the screen's own too.
  tools: OwnChanges,
}
impl Component for ToolsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
      retry: ctx.signal(0),
      tools: OwnChanges::new(ctx, &[Outside::Tools]),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let id = match props.page {
      ProjectPage::Tool(id, _) => Some(id),
      _ => None,
    };
    let root = props.project.open.repo_root.clone();
    let revision = self.revision.get();
    let outside = props.project.feed.outside.get();
    let outside = (outside.pipelines, self.tools.count(&props.project.feed));
    let reading = ctx
      .future(
        (root, id, revision, outside, self.retry.get()),
        |(root, id, revision, ..)| {
          tracing::trace!(target: crate::app_log::READ_LOG, read = "tools");
          async move {
            background::run(move || EditorData::load_in(&root, id))
              .await?
              .map(|data| (id, revision, data))
              .map_err(|error| messages::store_error(&error))
          }
        },
      )
      .state()
      .get();
    let frame = Column::new()
      .id("screen-tools")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    let error = reading.error.as_ref().filter(|_| !reading.is_pending());
    if matches!(props.page, ProjectPage::Tools)
      && let Some(error) = error
    {
      return frame.child(self.library(ctx, props.project, Err(error.clone())));
    }
    let current = reading
      .data
      .filter(|(read_id, read_revision, _)| *read_id == id && *read_revision == revision);
    let Some((_, _, data)) = current else {
      return match error {
        Some(error) => frame.child(self.failed(ctx, error)),
        None => frame.child(loading("tools-loading", &ctx.t("tools.reading"))),
      };
    };
    let child = self.content(ctx, props, data, id);
    frame.with_children(error.map(|error| self.failed(ctx, error))).child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(child),
    )
  }
}
impl ToolsScreen {
  fn content(&self, ctx: &mut Ctx, props: Props, data: EditorData, id: Option<ResourceId>) -> Element {
    match props.page {
      ProjectPage::Tools => self.library(ctx, props.project, Ok(data.listed)),
      ProjectPage::NewTool | ProjectPage::Tool(..) => {
        if id.is_some_and(|id| data.listed.tool(id).is_none()) {
          return crate::ui::notice::Notice::new(
            "tool-missing",
            crate::ui::notice::NoticeTone::Danger,
            ctx.t("tools.notFound"),
          )
          .into();
        }
        let revision = self.revision.get();
        let entry = id.and_then(|id| data.listed.tool(id)).cloned();
        let key = format!(
          "tool-editor-{}-{revision}",
          id.map_or_else(|| "new".to_owned(), |id| id.to_string())
        );
        ctx.mount_keyed::<ToolEditor>(
          &key,
          EditorProps {
            project: props.project,
            data,
            entry,
            revision: self.revision.clone(),
            tools: self.tools.clone(),
            read_revision: self.retry.get(),
            page: props.page,
          },
        )
      }
      _ => unreachable!("Tools mounts only its own pages"),
    }
  }
  fn library(
    &self,
    ctx: &mut Ctx,
    project: ShownProject,
    listed: Result<orchester_workspace::tools::data::ProjectTools, Message>,
  ) -> Element {
    ctx.mount_keyed::<library::Library>(
      "tools-library",
      library::Props {
        project,
        listed,
        retry: self.retry.clone(),
      },
    )
  }
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let retry = self.retry.clone();
    load_error(
      ctx,
      "tools-load-error",
      (&ctx.t("tools.loadFailed"), &message_text(ctx, error)),
      move || retry.update(|r| *r += 1),
    )
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
