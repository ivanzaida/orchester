//! Instructions, the Agents hub's second tab (`02 Instructions` `jtiqe`): one list in the order prompts assemble them,
//! a rule's page with its prompt preview and variables beside it (`02b` `Sn37u`, `02d` `JXvc0`), a new rule that
//! chooses its scope on the page (`02e` `dMYPx`), and the import page (`02c` `P21bwh`). The rule shown and the panel
//! beside it are the route's ([`View`]), so Back and Forward step through them.
mod autosave;
mod availability;
mod editor;
mod editor_actions;
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
mod page_head;
mod panels;
mod preview;
mod rule;
mod scope_choice;
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
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{background, instructions::data::ProjectInstructions, messages, tasks::outside::Outside};
use state::{Selection, Tab, View};

use crate::{
  project::ShownProject,
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
pub struct Props(pub ShownProject, pub View);
impl DevtoolsInspectable for Props {}
pub struct InstructionsScreen {
  selection: Selection,
  retry: Signal<u64>,
  /// Counts the person's reloads of an instruction: each reads the list again too, for a change nothing
  /// recorded.
  reloads: Signal<u64>,
  leave: crate::navigation::guard::LeaveGuard,
  /// Why the instructions couldn't be read, kept while they are read again.
  failure: LastFailure,
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
    let selection = Selection {
      project: ctx.props::<Props>().0.open.project.id(),
      navigation: crate::navigation::Navigation::of(ctx),
      pending: ctx.signal(None),
      created: ctx.signal(state::Saved::default()),
      dirty,
      busy,
      removed: ctx.signal(state::Removed::default()),
    };
    let rule = selection.clone();
    let leave = selection.navigation.guard_within(
      move || edits.get_untracked() || writing.get_untracked(),
      move |route| rule.holds(route),
    );
    Self {
      selection,
      retry: ctx.signal(0),
      reloads: ctx.signal(0),
      leave,
      failure: LastFailure::default(),
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
    let view = ctx.props::<Props>().1;
    let head = self.head(ctx, view);
    let retry = self.retry.clone();
    let what = ctx.t("instructions.loadFailed");
    let failure = hub::read_failure(
      ctx,
      &project,
      (HubTab::Instructions, "instructions-load-error", &what),
      self.failure.follow(&read),
      move || retry.update(|v| *v += 1),
    );
    let failure = match failure {
      Some(failure) if read.data.is_none() => {
        let body = hub::list_body("instructions-failed", failure.page(ctx));
        return frame(ctx, &project, head, body);
      }
      failure => failure,
    };
    if read.is_pending() && read.data.is_none() {
      let body = loading("instructions-loading", &ctx.t("instructions.reading"));
      return frame(ctx, &project, head, body.into());
    }
    let mut page = Column::new().width(Dimension::Pct(100.0)).height(Dimension::Pct(100.0));
    if let Some(failure) = failure {
      // What was read stays under it.
      page = page.child(
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
    let reading = read.is_pending();
    if let Some(data) = read.data {
      page = page
        .with_children(leaving::question(ctx, &self.selection, &self.leave))
        .child(
          Column::new()
            .width(Dimension::Pct(100.0))
            .flex(1.0)
            .min_height(Dimension::Px(0.0))
            .child(self.content(ctx, (project.clone(), view), data, reading)),
        );
    }
    frame(ctx, &project, head, page.into())
  }
}
impl InstructionsScreen {
  /// Import from repository and New instruction, in the scope of the rule shown.
  fn head(&self, ctx: &Ctx, view: View) -> Head {
    let tab = view.tab().unwrap_or_default();
    Head::actions([
      ActionButton::new("instructions-import", ButtonKind::Ghost, ctx.t("instructions.import"))
        .icon(Icon::FolderOpen)
        .opens(self.selection.to(View::Import))
        .into(),
      ActionButton::new("instructions-new", ButtonKind::Primary, ctx.t("instructions.new"))
        .icon(Icon::Plus)
        .opens(self.selection.to(View::new_rule(tab)))
        .into(),
    ])
  }

  fn content(
    &self,
    ctx: &mut Ctx,
    (project, view): (ShownProject, View),
    data: ProjectInstructions,
    reading: bool,
  ) -> Element {
    let created = self.selection.created.get().0;
    if created.as_ref().is_some_and(|entry| data.entry(entry.id).is_some()) {
      self.selection.created.set(state::Saved::default());
    }
    let (tab, entry, panel) = match view {
      View::Import => {
        return ctx.mount_keyed::<import::Import>(
          "instruction-import",
          import::Props {
            project,
            selection: self.selection.clone(),
          },
        );
      }
      View::Rule(_, None, _) if data.entries.is_empty() => {
        return frame::empty(ctx, &project, (&data, Tab::Project), &self.selection);
      }
      View::Rule(_, None, _) => return library::list(ctx, &data, &self.selection),
      View::Rule(tab, Some(id), panel) => {
        let entry = data
          .entry(id)
          .or_else(|| created.as_ref().filter(|entry| entry.id == id))
          .cloned();
        if entry.is_none() {
          return library::list(ctx, &data, &self.selection);
        }
        (tab, entry, panel)
      }
      View::New(tab, panel) => (tab, None, panel),
    };
    let key = format!("instruction-editor-{:?}", entry.as_ref().map(|entry| entry.id));
    ctx.mount_keyed::<editor::Editor>(
      &key,
      editor::Props {
        project,
        data,
        entry,
        tab,
        panel,
        selection: self.selection.clone(),
        reloads: self.reloads.clone(),
        reading,
      },
    )
  }
}

/// The hub on its Instructions tab around `body`, as the screen's element.
fn frame(ctx: &mut Ctx, project: &ShownProject, head: Head, body: Element) -> Element {
  Column::new()
    .id("screen-instructions")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(hub::page(ctx, project, (HubTab::Instructions, head), body))
    .into()
}
