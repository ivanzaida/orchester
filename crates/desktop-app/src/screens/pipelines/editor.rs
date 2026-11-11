//! A pipeline's page (`02 · Pipeline` `IsU5u`): the edit of its latest version under its header and tabs, the Steps
//! tab's flow and column, the Settings tab, the Versions tab, or a viewed version over the flow; with the questions,
//! problems and failures in place under the header (`05b` `eCrOM`). Nothing saves on its own: a pipeline holds no
//! draft (D-032), so *Save* publishes the next version.

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
use orchester_core::resource::{EditProblem, Pipeline, PipelineEdit, VersionNumber};
use orchester_workspace::{
  background, messages,
  pipelines::{data::OpenedPipeline, history::VersionHistory, problems::problem_view, write::needs_reload},
  sharing::team::TeamConfiguration,
};

use super::{
  actions::Writes,
  parts,
  state::{Editing, State, Tab, View, Views},
  work::{self, Action},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  screens::hub,
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
  /// The published version the route views, read-only; `None` for the edit.
  pub version: Option<VersionNumber>,
  pub reload: Signal<u64>,
  /// The steps of the pipeline's version taken back from the team server, which the page holds as its edit, not saved
  /// (D-032), once it is read again.
  pub taken_back: Signal<Option<Pipeline>>,
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
pub struct Editor {
  state: State,
  action: Action,
  confirm_save: Signal<bool>,
  writing: Signal<bool>,
  leave: crate::navigation::guard::LeaveGuard,
}
impl Component for Editor {
  type Props = EditorProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let navigation = Navigation::of(ctx);
    let views = Views::new(navigation.clone(), props.project.open.project.id(), props.opened.id);
    let mut editing = Editing::new(props.opened, (views.clone(), props.view));
    if let Some(steps) = props.taken_back.get_untracked() {
      props.taken_back.set(None);
      editing.edit = PipelineEdit::new(steps);
    }
    let state = ctx.signal(editing);
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
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    ctx.provide(super::sources::EditorProject(props.project.open.clone()));
    ctx.provide(super::leaving::Leaving(self.leave.clone()));
    let writing = self.action.state().get().is_pending();
    if self.writing.get_untracked() != writing {
      self.writing.set(writing);
    }
    // The view is the route's; the state follows it for the parts that read it there.
    if self.state.get_untracked().view != props.view {
      self.state.update(|s| s.view = props.view.clone());
    }
    let reading_version = self.follow_version(ctx, &props);
    let opened = self.state.get().opened;
    props.project.feed.pipeline_named(opened.id, &opened.name);
    let editing = self.state.get();
    let locked = self.confirm_save.get() || props.loading || reading_version;
    let busy = writing || locked;
    let team = hub::team::of(&props.project);
    let may_change = hub::team::may_change(team.as_ref());
    let writes = Writes {
      state: self.state.clone(),
      action: self.action.clone(),
      root: props.project.open.repo_root.clone(),
      asking: self.confirm_save.clone(),
    };
    ctx.provide(super::actions::ColumnWrites(writes.clone()));
    let shared = team.as_ref().and_then(|team| {
      let standing = team.standing(editing.opened.id)?;
      // A pipeline holds no draft (D-032).
      Some(hub::team::head_line(ctx, "pipeline-team-state", team, standing, None))
    });
    // A member of a shared project does not save the team's pipeline (03).
    let save = may_change.then(|| writes.save_button(ctx, &editing, busy));
    let head = super::header::header(
      ctx,
      &self.state,
      save,
      // While the save question waits, nothing is being saved: the status says the changes are unsaved.
      (writing || props.loading || reading_version, shared),
    );
    let mut notices = self.team_notice(ctx, &props, team.as_ref());
    notices.extend(self.notices(ctx, &props, (&writes, writing)));
    // A member's page is locked as one being saved is: its fields take no edit.
    let content = self.content(ctx, &props, (&writes, (locked || !may_change, busy)));
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
  /// In a shared project, under the header: who may change the pipeline for a member (03), and for an admin a version
  /// published here that waits (02b, 02c).
  fn team_notice(&self, ctx: &mut Ctx, props: &EditorProps, team: Option<&TeamConfiguration>) -> Vec<Element> {
    let Some(team) = team else {
      return Vec::new();
    };
    if !team.may_change {
      return vec![hub::team::line(ctx, "pipeline-team-line", team).into()];
    }
    let Some(standing) = team.standing(props.opened.id) else {
      return Vec::new();
    };
    let (reload, taken) = (props.reload.clone(), props.taken_back.clone());
    // The page reads the saved pipeline again, as *Reload saved pipeline* does, holding the steps taken back.
    let reread: hub::team::Reread = std::sync::Arc::new(move || reload.update(|n| *n += 1));
    let hand_back: hub::team::HandBack = std::sync::Arc::new(move |steps| taken.set(Some(steps)));
    // Taking the version back would replace an edit not saved, so the bar does not while there is one (D-040).
    let holds_edit = self.state.get().unsaved();
    hub::team::pipeline_waiting_bar(ctx, &props.project, (team, standing), (reread, hand_back, holds_edit))
      .into_iter()
      .collect()
  }

  /// What sits in place under the header: what an import did, leaving, the save question, problems and failures.
  fn notices(&self, ctx: &mut Ctx, props: &EditorProps, (writes, writing): (&Writes, bool)) -> Vec<Element> {
    let editing = self.state.get();
    let mut notices: Vec<Element> = Vec::new();
    // A copy shows what its import did until it is published (`H2g3Hs` in `ky5kJ`).
    if let Some(imported) = editing
      .opened
      .imported
      .as_ref()
      .filter(|imported| imported.shows_result(true))
    {
      notices.push(super::imported::strip(ctx, props.project.open.project.id(), imported));
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
      writes.publish_column(ctx)
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
    if self.confirm_save.get() || self.leave.requested() || editing.view == View::Publish {
      return content;
    }
    super::publication::with_success(ctx, &self.state, content)
  }

  /// The published version shown is the route's: Back or Forward to a place of the edit shows the edit again,
  /// editable, and to a place of a version reads that version once more. `true` while it is read; the page waits
  /// then. The first of the futures each render asks for: lurq keeps a component's futures by that order.
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
        s.epoch += 1;
      }),
      Some(version) if shown != Some(version) => {
        let (pending, failed) = (read.is_pending(), read.error.is_some());
        let Some(found) = read.data.filter(|_| !pending && !failed) else {
          return !failed;
        };
        // A version the history no longer has shows the edit.
        if let Some((number, body)) = found.filter(|(number, _)| *number == version) {
          self.state.update(|s| {
            s.viewing = Some(body);
            s.viewing_version = Some(number);
            s.epoch += 1;
          });
        }
      }
      _ => {}
    }
    false
  }
}

/// The fields' errors and the rules the edit breaks that keep it from being saved.
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
  // The rules the steps break; a step without a profile or tool shows in the publish strip only.
  let broken = |problem: &&EditProblem| matches!(problem, EditProblem::Cycle(_) | EditProblem::Rule(_));
  for problem in editing.edit.problems().iter().filter(broken) {
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
