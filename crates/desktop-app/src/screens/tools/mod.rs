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
use orchester_workspace::{
  background,
  messages::{self, Message},
  tools::editor::EditorData,
};

use crate::{
  project::ShownProject,
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
  revision: Signal<u64>,
}
impl Component for ToolsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
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
    let outside = (outside.pipelines, outside.tools);
    let reading = ctx
      .future((root, id, revision, outside), |(root, id, revision, _)| async move {
        background::run(move || EditorData::load_in(&root, id))
          .await?
          .map(|data| (id, revision, data))
          .map_err(|error| messages::store_error(&error))
      })
      .state()
      .get();
    let frame = Column::new()
      .id("screen-tools")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    if let Some(error) = reading.error.as_ref().filter(|_| !reading.is_pending()) {
      return frame.child(self.failed(ctx, error));
    }
    let Some((read_id, read_revision, data)) = reading.data else {
      return frame.child(loading("tools-loading", &ctx.t("tools.reading")));
    };
    if read_id != id || read_revision != revision {
      return frame.child(loading("tools-loading", &ctx.t("tools.reading")));
    }
    let child = match props.page {
      ProjectPage::Tools => ctx.mount_keyed::<library::Library>(
        "tools-library",
        library::Props {
          project: props.project,
          listed: data.listed,
        },
      ),
      ProjectPage::NewTool | ProjectPage::Tool(..) => {
        if id.is_some_and(|id| data.listed.tool(id).is_none()) {
          return frame.child(crate::ui::notice::Notice::new(
            "tool-missing",
            crate::ui::notice::NoticeTone::Danger,
            ctx.t("tools.notFound"),
          ));
        }
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
            page: props.page,
          },
        )
      }
      _ => unreachable!("Tools mounts only its own pages"),
    };
    frame.child(child)
  }
}
impl ToolsScreen {
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let revision = self.revision.clone();
    load_error(
      ctx,
      "tools-load-error",
      (&ctx.t("tools.loadFailed"), &message_text(ctx, error)),
      move || revision.update(|r| *r += 1),
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
