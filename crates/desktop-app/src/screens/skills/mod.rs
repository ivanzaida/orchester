//! Skills library and editor, Skills layer TOntf in orchester2.pen.
mod editor;
mod import_preview;
mod inspector;
mod library;
mod repository;
mod repository_list;
mod scope;
mod state;
mod words;
mod work;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{background, skills::editor::EditorData};
use state::{Panel, View};

use crate::{
  project::ShownProject,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};
#[derive(Clone, PartialEq)]
pub struct Props {
  project: ShownProject,
  page: crate::routes::ProjectPage,
}
impl DevtoolsInspectable for Props {}
struct SkillsScreen {
  view: View,
}
impl Component for SkillsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      view: View {
        panel: state::PanelNav {
          project: ctx.props::<Props>().project.open.project.id(),
          navigation: crate::navigation::Navigation::of(ctx),
        },
        revision: ctx.signal(0),
        reset: ctx.signal(0),
      },
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let root = project.open.repo_root.clone();
    let outside = project.feed.outside.get();
    let reading = ctx
      .future(
        (
          root,
          self.view.revision.get(),
          self.view.reset.get(),
          outside.profiles,
          outside.pipelines,
          outside.project_settings,
          outside.runs,
          project.feed.tasks.get(),
        ),
        |(root, _, reset, ..)| async move {
          background::run(move || EditorData::load_in(&root))
            .await?
            .map(|data| (data, reset))
        },
      )
      .state()
      .get();
    let mut frame = Column::new()
      .id("screen-skills")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    let Some((data, read_reset)) = reading.data.clone() else {
      return frame.child(self.awaiting(ctx, reading.error.as_ref().filter(|_| !reading.is_pending()), false));
    };
    if let Some(error) = reading.error.as_ref().filter(|_| !reading.is_pending()) {
      let revision = self.view.revision.clone();
      frame = frame.child(load_error(
        ctx,
        "skills-refresh-error",
        (&ctx.t("skills.refreshFailed"), &message_text(ctx, error)),
        move || revision.update(|r| *r += 1),
      ));
    }
    if read_reset != self.view.reset.get() {
      return frame.child(self.awaiting(ctx, reading.error.as_ref().filter(|_| !reading.is_pending()), true));
    }
    let props = PageProps {
      project,
      data,
      view: self.view.clone(),
    };
    let content = match self.view.panel.get() {
      Panel::Library => ctx.mount_keyed::<library::Library>("skills-library", props),
      Panel::Import => ctx.mount_keyed::<repository::Import>("skills-import", props),
      panel => {
        let entry = match panel {
          Panel::Edit(id) => {
            let Some(entry) = props.data.listed.entry(id).cloned() else {
              return frame.child(if reading.is_pending() {
                Element::from(loading("skill-loading", &ctx.t("skills.reading")))
              } else {
                crate::ui::notice::Notice::new(
                  "skill-missing",
                  crate::ui::notice::NoticeTone::Danger,
                  ctx.t("skills.notFound"),
                )
                .into()
              });
            };
            Some(entry)
          }
          _ => None,
        };
        let key = entry.as_ref().map_or_else(
          || "skill-new".to_owned(),
          |e| format!("skill-editor-{}-{}", e.id, self.view.reset.get()),
        );
        ctx.mount_keyed::<editor::Editor>(&key, editor::Props { page: props, entry })
      }
    };
    frame.child(content)
  }
}
impl SkillsScreen {
  fn awaiting(&self, ctx: &Ctx, error: Option<&orchester_workspace::messages::Message>, reload: bool) -> Element {
    if let Some(error) = error {
      let revision = self.view.revision.clone();
      load_error(
        ctx,
        if reload {
          "skills-reload-error"
        } else {
          "skills-load-error"
        },
        (&ctx.t("skills.loadFailed"), &message_text(ctx, error)),
        move || revision.update(|r| *r += 1),
      )
    } else {
      loading(
        if reload { "skill-reloading" } else { "skills-loading" },
        &ctx.t("skills.reading"),
      )
      .into()
    }
  }
}
#[derive(Clone)]
pub struct PageProps {
  pub project: ShownProject,
  pub data: EditorData,
  pub view: View,
}
impl PartialEq for PageProps {
  fn eq(&self, o: &Self) -> bool {
    self.project == o.project && self.data == o.data
  }
}
impl DevtoolsInspectable for PageProps {}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, page: &crate::routes::ProjectPage) -> Element {
  ctx.mount_keyed::<SkillsScreen>(
    &format!("skills-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      page: page.clone(),
    },
  )
}
