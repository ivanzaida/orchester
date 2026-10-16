//! Instructions fePhH: scope tabs, the library, editor and in-place preview.
mod autosave;
mod availability;
mod editor;
mod editor_notes;
mod empty;
mod fields;
mod frame;
mod import;
mod import_preview;
mod import_rows;
mod leaving;
mod library;
mod library_groups;
mod panels;
mod preview;
mod sources;
mod state;
mod variable_detail;
mod variables;
mod viewer;
mod words;
mod work;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{background, instructions::data::ProjectInstructions, messages, tasks::outside::Outside};
use state::{Selection, Tab, View};

use crate::{
  project::ShownProject,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
pub struct Props(pub ShownProject, pub View);
impl DevtoolsInspectable for Props {}
pub struct InstructionsScreen {
  selection: Selection,
  retry: Signal<u64>,
  /// Counts the person's reloads of an instruction: each reads the list again too, for a change nothing
  /// recorded.
  reloads: Signal<u64>,
  leave: crate::navigation::guard::LeaveGuard,
}
pub fn screen(ctx: &mut Ctx, project: &ShownProject, view: View) -> Element {
  ctx.mount_keyed::<InstructionsScreen>(
    &format!("instructions-{}", project.open.project.id()),
    Props(project.clone(), view),
  )
}
impl Component for InstructionsScreen {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let dirty = ctx.signal(false);
    let busy = ctx.signal(false);
    let (edits, writing) = (dirty.clone(), busy.clone());
    let navigation = crate::navigation::Navigation::of(ctx);
    let leave = navigation.guard(move || edits.get_untracked() || writing.get_untracked());
    Self {
      selection: Selection {
        project: ctx.props::<Props>().0.open.project.id(),
        navigation,
        pending: ctx.signal(None),
        created: ctx.signal(state::Saved::default()),
        dirty,
        busy,
        full: ctx.signal(false),
      },
      retry: ctx.signal(0),
      reloads: ctx.signal(0),
      leave,
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let project = ctx.props::<Props>().0.clone();
    let outside = project.feed.outside.get();
    let deps = (
      project.open.repo_root.clone(),
      self.retry.get() + self.reloads.get(),
      outside.of(Outside::Instructions),
      outside.of(Outside::Profiles),
      outside.of(Outside::Pipelines),
    );
    let hold = crate::services::Services::of(ctx).hold;
    let read = ctx
      .future(deps, move |(root, ..)| {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "instructions");
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || ProjectInstructions::load_in(&root))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      })
      .state()
      .get();
    let page = Column::new()
      .id("screen-instructions")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0));
    if read.is_pending() && read.data.is_none() {
      return page.child(loading("instructions-loading", &ctx.t("instructions.reading")));
    }
    let mut page = page;
    if let Some(error) = read.error.clone().filter(|_| read.is_rejected()) {
      let retry = self.retry.clone();
      page = page.child(load_error(
        ctx,
        "instructions-load-error",
        (&ctx.t("instructions.loadFailed"), &message_text(ctx, &error)),
        move || retry.update(|v| *v += 1),
      ));
    }
    let reading = read.is_pending();
    let Some(data) = read.data else {
      return page;
    };
    let view = ctx.props::<Props>().1;
    page
      .with_children(
        view
          .tab()
          .filter(|_| !self.selection.full.get())
          .map(|tab| frame::header(ctx, &project, &data, tab, &self.selection)),
      )
      .with_children(leaving::question(ctx, &self.selection, &self.leave))
      .child(self.content(ctx, (project, view), data, reading))
  }
}
impl InstructionsScreen {
  fn content(
    &self,
    ctx: &mut Ctx,
    (project, view): (ShownProject, View),
    data: ProjectInstructions,
    reading: bool,
  ) -> Element {
    match view.tab() {
      None => ctx.mount_keyed::<import::Import>(
        "instruction-import",
        import::Props {
          project,
          selection: self.selection.clone(),
        },
      ),
      Some(tab) => {
        let created = self.selection.created.get().0;
        if created.as_ref().is_some_and(|entry| data.entry(entry.id).is_some()) {
          self.selection.created.set(state::Saved::default());
        }
        let entry = match view {
          View::Rule(_, selected) => selected
            .and_then(|id| {
              data
                .entry(id)
                .or_else(|| created.as_ref().filter(|entry| entry.id == id))
            })
            .or_else(|| data.entries.iter().find(|entry| Tab::of(entry.body.scope()) == tab))
            .cloned(),
          _ => None,
        };
        if entry.is_none() && !matches!(view, View::New(_)) {
          return frame::empty(ctx, &project, (&data, tab), &self.selection);
        }
        let key = format!("instruction-editor-{tab:?}-{:?}", entry.as_ref().map(|entry| entry.id));
        ctx.mount_keyed::<editor::Editor>(
          &key,
          editor::Props {
            project,
            data,
            entry,
            tab,
            selection: self.selection.clone(),
            reloads: self.reloads.clone(),
            reading,
          },
        )
      }
    }
  }
}
