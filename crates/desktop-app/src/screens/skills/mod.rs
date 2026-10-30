//! Skills, the Agents hub's third tab (`03 Skills` `w0NiG0`): the skills by scope with what this repository holds, a
//! skill's page (`03b` `QbDWX`), and importing the repository's files with its result (`03c` `cC55E`).
mod editor;
mod inspector;
mod library;
mod repository;
mod scope;
mod state;
pub(crate) mod words;
mod work;
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
  },
  components::Column,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{background, messages::Message, skills::editor::EditorData, tasks::outside::Outside};
use state::{Panel, View};

use crate::{
  project::{ShownProject, own::OwnChanges},
  screens::hub::{self, Head, HubTab},
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    failure::LastFailure,
    icon::Icon,
    state::loading,
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
  accepted_reset: lurq::core::Signal<u64>,
  rejected_reset: lurq::core::Signal<u64>,
  /// Why the skills couldn't be read, kept while they are read again.
  failure: LastFailure,
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
        skills: OwnChanges::new(ctx, &[Outside::Skills]),
        reset: ctx.signal(0),
        reload_requested: ctx.signal(false),
      },
      accepted_reset: ctx.signal(0),
      rejected_reset: ctx.signal(0),
      failure: LastFailure::default(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().project.clone();
    let body = self.body(ctx, &project);
    // A member of a shared project adds and imports none of the team's skills.
    let head = if hub::team::may_change(hub::team::of(&project).as_ref()) {
      Head::actions([
        ActionButton::new("skills-import-open", ButtonKind::Ghost, ctx.t("skills.import"))
          .icon(Icon::FolderOpen)
          .opens(self.view.panel.to(Panel::Import))
          .into(),
        ActionButton::new("skills-new", ButtonKind::Primary, ctx.t("skills.new"))
          .icon(Icon::Plus)
          .opens(self.view.panel.to(Panel::New))
          .into(),
      ])
    } else {
      Head::default()
    };
    Column::new()
      .id("screen-skills")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(hub::page(ctx, &project, (HubTab::Skills, head), body))
  }
}
impl SkillsScreen {
  fn body(&self, ctx: &mut Ctx, project: &ShownProject) -> Element {
    let project = project.clone();
    let reading = self.read(ctx, &project);
    let failed = self.failure.follow(&reading);
    let mut frame = Column::new().width(Dimension::Pct(100.0)).height(Dimension::Pct(100.0));
    let Some((data, read_reset)) = reading.data.clone() else {
      return self.awaiting(ctx, &project, failed);
    };
    if reading.is_fulfilled() && !reading.is_pending() {
      self.accepted_reset.set(read_reset);
    }
    let refresh_failed = reading.error.clone().filter(|_| !reading.is_pending());
    if refresh_failed.is_some() {
      // Unlocking after rejection ends this discard decision. Later feed reads are passive.
      self.rejected_reset.set(self.view.reset.get_untracked());
    }
    let revision = self.view.revision.clone();
    let reload = self.view.reload_requested.clone();
    let replacing = read_reset != self.view.reset.get();
    let what = ctx.t("skills.refreshFailed");
    let failure = hub::read_failure(
      ctx,
      &project,
      (HubTab::Skills, "skills-refresh-error", &what),
      refresh_failed.map(|error| (error, false)),
      move || {
        if replacing {
          reload.set(true);
        } else {
          revision.update(|r| *r += 1);
        }
      },
    );
    if let Some(failure) = failure {
      // What was read stays under it.
      frame = frame.child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(
            Padding::new()
              .left(HubSpacing::Side.role())
              .right(HubSpacing::Side.role())
              .top(HubSpacing::BodyTop.role()),
          )
          .child(failure.page(ctx)),
      );
    }
    let requested = self.view.reset.get();
    let reloading = requested > self.rejected_reset.get_untracked() && requested != read_reset && reading.is_pending();
    if reloading {
      frame = frame.child(loading("skill-reloading", &ctx.t("skills.reading")));
    }
    let panel = Panel::of(&ctx.props::<Props>().page);
    let props = PageProps {
      project,
      data,
      view: self.view.clone(),
      reloading,
      filter: match panel {
        Panel::Library(filter) => filter,
        _ => crate::routes::SkillsFilter::default(),
      },
    };
    let content = Self::content(ctx, &panel, props, (read_reset, reading.is_pending()));
    frame
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(content),
      )
      .into()
  }

  /// The list, the import page or a skill's page, as `panel` names it; a skill no longer there says so.
  fn content(ctx: &mut Ctx, panel: &Panel, props: PageProps, (read_reset, pending): (u64, bool)) -> Element {
    let entry = match *panel {
      Panel::Library(_) => return ctx.mount_keyed::<library::Library>("skills-library", props),
      Panel::Import => return ctx.mount_keyed::<repository::Import>("skills-import", props),
      Panel::Edit(id) => match props.data.listed.entry(id).cloned() {
        Some(entry) => Some(entry),
        None if pending => return library::reading(ctx),
        None => {
          return crate::ui::notice::Notice::new(
            "skill-missing",
            crate::ui::notice::NoticeTone::Danger,
            ctx.t("skills.notFound"),
          )
          .into();
        }
      },
      Panel::New => None,
    };
    let key = entry.as_ref().map_or_else(
      || format!("skill-new-{read_reset}"),
      |e| format!("skill-editor-{}-{read_reset}", e.id),
    );
    ctx.mount_keyed::<editor::Editor>(&key, editor::Props { page: props, entry })
  }
}
impl SkillsScreen {
  fn read(&self, ctx: &mut Ctx, project: &ShownProject) -> FutureState<(EditorData, u64), Message> {
    let root = project.open.repo_root.clone();
    let outside = project.feed.outside.get();
    let hold = crate::services::Services::of(ctx).hold;
    let accepted = self.accepted_reset.get_untracked();
    let rejected = self.rejected_reset.get_untracked();
    ctx
      .future(
        (
          root,
          self.view.revision.get(),
          self.view.reset.get(),
          outside.profiles,
          outside.pipelines,
          outside.project_settings,
          outside.runs,
          self.view.skills.count(&project.feed),
          // The skills' active uses: the runs that hold them start and end, and their worktrees go.
          project.feed.states.get(),
          project.feed.job_state.get(),
        ),
        move |(root, _, reset, ..)| {
          tracing::trace!(target: crate::app_log::READ_LOG, read = "skills");
          let generation = if reset > rejected { reset } else { accepted };
          let hold = hold.clone();
          async move {
            hold.released().await;
            background::run(move || EditorData::load_in(&root))
              .await?
              .map(|data| (data, generation))
          }
        },
      )
      .state()
      .get()
  }
  /// Before the first read: the loader, or why it couldn't be read with Try again.
  fn awaiting(&self, ctx: &mut Ctx, project: &ShownProject, failed: Option<(Message, bool)>) -> Element {
    let revision = self.view.revision.clone();
    let what = ctx.t("skills.loadFailed");
    match hub::read_failure(
      ctx,
      project,
      (HubTab::Skills, "skills-load-error", &what),
      failed,
      move || revision.update(|r| *r += 1),
    ) {
      Some(failure) => hub::list_body("skills-failed", failure.page(ctx)),
      None => library::reading(ctx),
    }
  }
}
#[derive(Clone)]
pub struct PageProps {
  pub project: ShownProject,
  pub data: EditorData,
  pub view: View,
  pub reloading: bool,
  /// The library's filter, as the route names it.
  pub filter: crate::routes::SkillsFilter,
}
impl PartialEq for PageProps {
  fn eq(&self, o: &Self) -> bool {
    self.project == o.project && self.data == o.data && self.reloading == o.reloading && self.filter == o.filter
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
