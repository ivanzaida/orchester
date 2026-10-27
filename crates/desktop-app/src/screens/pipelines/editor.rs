//! A pipeline's page (`02 · Pipeline` `IsU5u`): the loaded draft under its header and tabs, the Steps tab's flow
//! and column, the Settings tab, the Versions tab, or a viewed version over the flow; with the questions, problems
//! and failures in place under the header (`05b` `eCrOM`). The draft saves itself once its fields rest.
use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{
  background, messages,
  pipelines::{
    data::OpenedPipeline,
    history::VersionHistory,
    problems::problem_view,
    widening::draft_reasons,
    write::{Allowed, needs_reload},
  },
};
use parking_lot::Mutex;

use super::{
  actions::Writes,
  parts,
  state::{Editing, State, Tab, View, Views},
  work::{self, Action, Work},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

#[derive(Clone)]
pub struct EditorProps {
  pub project: ShownProject,
  pub opened: OpenedPipeline,
  /// What the route shows.
  pub view: View,
  /// The published version the route views, read-only; `None` for the draft.
  pub version: Option<VersionNumber>,
  pub reload: Signal<u64>,
  pub loading: bool,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.opened == other.opened
      && self.view == other.view
      && self.version == other.version
      && self.loading == other.loading
  }
}
impl DevtoolsInspectable for EditorProps {}
/// What autosave writes once the fields rest: the body and name, into the project at the path.
#[derive(Clone, PartialEq)]
struct Snapshot(Option<(orchester_core::resource::Pipeline, orchester_core::name::Name, PathBuf)>);
impl DevtoolsInspectable for Snapshot {}
pub struct Editor {
  state: State,
  action: Action,
  confirm_save: Signal<bool>,
  writing: Signal<bool>,
  leave: crate::navigation::guard::LeaveGuard,
  /// Whether autosave's rest is followed already; once per editor.
  followed: Mutex<bool>,
}
impl Component for Editor {
  type Props = EditorProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let navigation = Navigation::of(ctx);
    let views = Views::new(navigation.clone(), props.project.open.project.id(), props.opened.id);
    let state = ctx.signal(Editing::new(props.opened, (views.clone(), props.view)));
    let action = work::action(ctx, props.project.open.project.id());
    follow_writes(ctx, &action, &state);
    let writing = ctx.signal(false);
    let (edits, pending) = (state.clone(), writing.clone());
    let leave = navigation.guard_within(
      move || edits.get_untracked().unsaved() || pending.get_untracked(),
      move |route| views.holds(route),
    );
    Self {
      state,
      action,
      writing,
      leave,
      confirm_save: ctx.signal(false),
      followed: Mutex::new(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    ctx.provide(super::sources::EditorProject(props.project.open.clone()));
    let writing = self.action.state().get().is_pending();
    if self.writing.get_untracked() != writing {
      self.writing.set(writing);
    }
    // The view is the route's; the state follows it for the parts that read it there.
    if self.state.get_untracked().view != props.view {
      self.state.update(|s| s.view = props.view.clone());
    }
    // First: lurq keeps a component's futures by the order it asks for them.
    self.autosave(ctx, &props, writing);
    let reading_version = self.follow_version(ctx, &props);
    let opened = self.state.get().opened;
    props.project.feed.pipeline_named(opened.id, &opened.name);
    let editing = self.state.get();
    let locked = self.confirm_save.get() || props.loading || reading_version;
    let busy = writing || locked;
    let writes = Writes {
      state: self.state.clone(),
      action: self.action.clone(),
      root: props.project.open.repo_root.clone(),
      asking: self.confirm_save.clone(),
    };
    ctx.provide(super::actions::ColumnWrites(writes.clone()));
    let head = super::header::header(
      ctx,
      &self.state,
      (
        writes.save_button(ctx, &editing, busy),
        writes.publish_button(ctx, &editing, busy),
      ),
      // While the save question waits, nothing is being saved: the status says the changes are unsaved.
      writing || props.loading || reading_version,
    );
    let notices = self.notices(ctx, &props, (&writes, writing));
    let content = self.content(ctx, &props, (&writes, (locked, busy)));
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .with_children(notices)
      .child(content);
    let page = parts::page().child(head).child(parts::body("pipeline-body", body));
    self.with_publication_notice(ctx, page.into())
  }
}
impl Editor {
  /// What sits in place under the header: what an import did, leaving, the save question, problems and failures.
  fn notices(&self, ctx: &mut Ctx, props: &EditorProps, (writes, writing): (&Writes, bool)) -> Vec<Element> {
    let editing = self.state.get();
    let mut notices = Vec::new();
    // A copy shows what its import did until it is published (`H2g3Hs` in `ky5kJ`).
    if let Some(imported) = editing
      .opened
      .imported
      .as_ref()
      .filter(|imported| imported.shows_result(editing.opened.latest.is_some()))
    {
      notices.push(crate::screens::import::imported::strip(
        ctx,
        props.project.open.project.id(),
        imported,
      ));
    }
    notices.extend(super::leaving::question(
      ctx,
      &self.leave,
      writing,
      editing.name.as_str(),
    ));
    // On the Steps tab the column asks it, under its title (`02e` `T46Jq`).
    if self.confirm_save.get() && !super::actions::asks_in_column(&editing) {
      notices.push(writes.save_question(ctx));
    }
    notices.extend(super::publication::problems(ctx, &self.state));
    notices.extend(self.write_failure(ctx, props));
    notices.extend(errors(ctx, &editing));
    notices
  }

  /// A write that failed, and *Reload saved pipeline* when the saved one changed meanwhile.
  fn write_failure(&self, ctx: &Ctx, props: &EditorProps) -> Option<Element> {
    let error = self.action.state().get().error?;
    let notice = Notice::new("pipeline-write-error", NoticeTone::Danger, ctx.t("pipelines.error"))
      .stacked()
      .flat()
      .body(message_text(ctx, &error));
    if !needs_reload(&error) {
      return Some(notice.into());
    }
    let reload = props.reload.clone();
    Some(
      notice
        .part(parts::note(&ctx.t("pipelines.conflict")))
        .action(
          ActionButton::new("pipeline-reload", ButtonKind::Secondary, ctx.t("pipelines.reload"))
            .on_click(move || reload.update(|n| *n += 1)),
        )
        .into(),
    )
  }

  /// The open tab, or the viewed version over its flow.
  fn content(&self, ctx: &mut Ctx, props: &EditorProps, (writes, (locked, busy)): (&Writes, (bool, bool))) -> Element {
    let editing = self.state.get();
    if editing.view.tab() == Tab::Settings {
      return super::settings::page(ctx, &self.state, locked);
    }
    if editing.viewing.is_some() {
      return ctx.mount_keyed::<super::viewing::Viewed>(
        "pipeline-viewed",
        super::viewing::Props {
          project: props.project.clone(),
          state: self.state.clone(),
          locked: (locked, busy),
        },
      );
    }
    if editing.view.tab() == Tab::Versions {
      return ctx.mount_keyed::<super::versions::Versions>(
        "pipeline-versions",
        super::versions::Props {
          project: props.project.clone(),
          state: self.state.clone(),
        },
      );
    }
    let column = if editing.view == View::Publish {
      writes.publish_column(ctx, busy)
    } else {
      super::column::column(ctx, &self.state, (locked, busy))
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Section)
      .align_items(Alignment::Start)
      .child(super::flow::flow(ctx, &self.state, busy))
      .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(column))
      .into()
  }

  fn with_publication_notice(&self, ctx: &mut Ctx, content: Element) -> Element {
    let editing = self.state.get();
    if self.confirm_save.get() || self.leave.requested() || editing.view == View::Publish || editing.restore_requested {
      return content;
    }
    super::publication::with_success(ctx, &self.state, content)
  }

  /// The published version shown is the route's: Back or Forward to a place of the draft shows the draft again,
  /// editable, and to a place of a version reads that version once more. `true` while it is read; the page waits
  /// then. Second of the futures each render asks for.
  fn follow_version(&self, ctx: &mut Ctx, props: &EditorProps) -> bool {
    let shown = self.state.get_untracked().viewing_version;
    let wanted = props.version.filter(|version| shown != Some(*version));
    let read = ctx
      .future(
        (props.project.open.repo_root.clone(), props.opened.id, wanted),
        |(root, id, wanted)| async move {
          let Some(number) = wanted else {
            return Ok(None);
          };
          background::run(move || {
            VersionHistory::load_in(&root, id)
              .map(|history| history.version(number).map(|version| (number, (*version.body).clone())))
              .map_err(|e| messages::store_error(&e))
          })
          .await?
        },
      )
      .state()
      .get();
    match props.version {
      None if shown.is_some() => self.state.update(|s| {
        s.viewing = None;
        s.viewing_version = None;
        s.comparing = false;
        s.restore_requested = false;
        s.epoch += 1;
      }),
      Some(version) if shown != Some(version) => {
        let (pending, failed) = (read.is_pending(), read.error.is_some());
        let Some(found) = read.data.filter(|_| !pending && !failed) else {
          return !failed;
        };
        // A version the history no longer has shows the draft.
        if let Some((number, body)) = found.filter(|(number, _)| *number == version) {
          self.state.update(|s| {
            s.viewing = Some(body);
            s.viewing_version = Some(number);
            s.comparing = false;
            s.restore_requested = false;
            s.epoch += 1;
          });
        }
      }
      _ => {}
    }
    false
  }

  /// Saves the fields once they rest valid and unchanged for a moment. The rest is the first future each render asks
  /// for, so lurq keeps its state across renders and following it once follows every rest. No rest runs while a write
  /// is `writing`: what was edited meanwhile rests once the write ends, so it is saved after it rather than skipped.
  fn autosave(&self, ctx: &mut Ctx, props: &EditorProps, writing: bool) {
    let editing = self.state.get();
    let root = props.project.open.repo_root.clone();
    let body = (editing.dirty()
      && !writing
      && editing.errors.is_empty()
      && editing.viewing.is_none()
      && !editing.restore_requested
      && !props.loading
      && !self.leave.requested())
    .then(|| {
      editing
        .edit
        .validate()
        .ok()
        .map(|body| (body, editing.name.clone(), root))
    })
    .flatten();
    let ready = ctx
      .future(body, |body| async move {
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        Ok::<_, orchester_workspace::messages::Message>(Snapshot(body))
      })
      .state();
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let (state, action, leave) = (self.state.clone(), self.action.clone(), self.leave.clone());
    ctx.watch(&ready, move |ready| {
      let Some(Snapshot(Some((body, name, root)))) = &ready.data else {
        return;
      };
      let current = state.get();
      let writing = action.state().get();
      if ready.is_pending()
        || leave.requested()
        || writing.is_pending()
        || writing.error.is_some()
        || current.viewing.is_some()
        || current.restore_requested
        || !current.dirty()
        || !current.errors.is_empty()
        || current.edit.validate().as_ref() != Ok(body)
        || &current.name != name
        || !draft_reasons(&current.opened, body).is_empty()
      {
        return;
      }
      action.run(Work::Save {
        root: root.clone(),
        name: current.name,
        before: current.opened,
        body: body.clone(),
        allowed: Allowed::default(),
      });
    });
  }
}

/// The fields' errors and the rules the draft breaks that keep it from being saved.
fn errors(ctx: &Ctx, editing: &Editing) -> Vec<Element> {
  // A step's Instructions page shows its text's error under the field only (`02g` `PexNQ`).
  let instructions = match &editing.view {
    View::StepPage(key, super::state::StepPage::Instructions) => Some(format!("{key}:step-instructions")),
    _ => None,
  };
  let mut errors = Vec::new();
  for (field, error) in &editing.errors {
    // The Settings tab shows the name's error on its field.
    if (field == "pipeline-name" && editing.view == View::Settings) || instructions.as_ref() == Some(field) {
      continue;
    }
    let message = if field == "pipeline-name" {
      parts::pipeline_name_error(ctx, error)
    } else {
      message_text(ctx, error)
    };
    errors.push(field_error("pipeline-field-error", &message).into());
  }
  for problem in editing.edit.problems().iter().filter(|p| p.blocks_saving()) {
    let view = problem_view(&editing.edit, problem, &ctx.t("pipelines.then"));
    errors.push(field_error("pipeline-rule-error", &message_text(ctx, &view.message)).into());
  }
  errors
}

/// Takes in what a fulfilled write stored, once per page: a failed write keeps the data of the one before it.
fn follow_writes(ctx: &mut Ctx, action: &Action, state: &State) {
  let state = state.clone();
  ctx.watch(&action.state(), move |result| {
    if !result.is_pending()
      && result.error.is_none()
      && let Some(opened) = &result.data
    {
      state.update(|state| state.saved(opened.clone()));
      let editing = state.get_untracked();
      if editing.view == View::Publish {
        editing.views.close();
      }
    }
  });
}
