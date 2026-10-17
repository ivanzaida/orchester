//! Loaded draft, graph and inspector, with explicit saves and an in-place publish review.
use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Row, Spacer},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{
  background, messages,
  pipelines::{
    data::OpenedPipeline,
    history::VersionHistory,
    problems::problem_view,
    widening::{draft_reasons, publish_reasons},
    write::{Allowed, next_version},
  },
};
use parking_lot::Mutex;

use super::{
  parts,
  state::{Editing, Panel, Panels, State},
  work::{self, Action, Work},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  screens::connections::widen::{WidenConfirm, WidenProps},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    message::message_text,
  },
};

#[derive(Clone)]
pub struct EditorProps {
  pub project: ShownProject,
  pub opened: OpenedPipeline,
  /// The panel the route shows.
  pub panel: Panel,
  /// The published version the route views, read-only; `None` for the draft.
  pub version: Option<VersionNumber>,
  pub reload: Signal<u64>,
  pub loading: bool,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.opened == other.opened
      && self.panel == other.panel
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
    let panels = Panels::new(navigation.clone(), props.project.open.project.id(), props.opened.id);
    let state = ctx.signal(Editing::new(props.opened, (panels.clone(), props.panel)));
    let action = work::action(ctx, props.project.open.project.id());
    follow_writes(ctx, &action, &state);
    let writing = ctx.signal(false);
    let (edits, pending) = (state.clone(), writing.clone());
    let leave = navigation.guard_within(
      move || {
        let edits = edits.get_untracked();
        edits.dirty() || !edits.errors.is_empty() || pending.get_untracked()
      },
      move |route| panels.holds(route),
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
    let action = self.action.clone();
    let writing = action.state().get().is_pending();
    if self.writing.get_untracked() != writing {
      self.writing.set(writing);
    }
    // The panel is the route's; the state follows it for the parts that read it there.
    if self.state.get_untracked().panel != props.panel {
      self.state.update(|s| s.panel = props.panel.clone());
    }
    // First: lurq keeps a component's futures by the order it asks for them.
    self.autosave(ctx, &props, writing);
    let reading_version = self.follow_version(ctx, &props);
    let opened = self.state.get().opened;
    props.project.feed.pipeline_named(opened.id, &opened.name);
    let editing = self.state.get();
    let busy = action.state().get().is_pending() || self.confirm_save.get() || props.loading || reading_version;
    let locked = self.confirm_save.get() || props.loading || reading_version;
    let head = self.header(ctx, &props, &action, busy);
    let content = if editing.panel == Panel::Settings {
      super::settings::page(ctx, &self.state, locked)
    } else {
      let graph = super::canvas::canvas(ctx, &self.state, busy);
      let panel = match &editing.panel {
        Panel::Publish => self.publish_panel(ctx, &props, &action, busy),
        Panel::History => ctx.mount_keyed::<super::history::History>(
          "pipeline-history",
          super::history::Props {
            project: props.project.clone(),
            state: self.state.clone(),
            writing: busy,
          },
        ),
        _ => super::inspector::panel(ctx, &self.state, locked, busy),
      };
      Row::new()
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(graph)
        .child(panel)
        .into()
    };
    let leave = super::leaving::question(ctx, &self.leave, writing, editing.name.as_str());
    let mut page = parts::page()
      .child(head)
      .with_children(leave)
      .with_children(super::publication::problems(ctx, &self.state))
      .child(content);
    if self.confirm_save.get() {
      page = page.child(self.save_question(ctx, &props, &action));
    }
    if let Some(error) = action.state().get().error {
      page = page.child(field_error("pipeline-write-error", &message_text(ctx, &error)));
      if orchester_workspace::pipelines::write::needs_reload(&error) {
        let reload = props.reload;
        page = page.child(parts::note(&ctx.t("pipelines.conflict"))).child(
          ActionButton::new("pipeline-reload", ButtonKind::Secondary, ctx.t("pipelines.reload"))
            .on_click(move || reload.update(|n| *n += 1)),
        );
      }
    }
    let page = self.errors(ctx, &editing, page);
    self.with_publication_notice(ctx, page.into())
  }
}
impl Editor {
  fn with_publication_notice(&self, ctx: &mut Ctx, content: Element) -> Element {
    let editing = self.state.get();
    if self.confirm_save.get() || self.leave.requested() || editing.panel == Panel::Publish || editing.restore_requested
    {
      return content;
    }
    super::publication::with_success(ctx, &self.state, content)
  }
  fn errors(&self, ctx: &Ctx, editing: &Editing, mut page: lurq::components::Column) -> lurq::components::Column {
    for (field, error) in &editing.errors {
      if field == "pipeline-name" && editing.panel == Panel::Settings {
        continue;
      }
      let message = if field == "pipeline-name" {
        parts::pipeline_name_error(ctx, error)
      } else {
        message_text(ctx, error)
      };
      page = page.child(field_error("pipeline-field-error", &message));
    }
    for problem in editing.edit.problems().iter().filter(|p| p.blocks_saving()) {
      let view = problem_view(&editing.edit, problem, &ctx.t("pipelines.then"));
      page = page.child(field_error("pipeline-rule-error", &message_text(ctx, &view.message)));
    }
    page
  }
  /// The published version shown is the route's: Back or Forward to a place of the draft shows the draft
  /// again, editable, and to a place of a version reads that version once more. `true` while it is read;
  /// the editor waits then. Second of the futures each render asks for.
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
  /// Saves the fields once they rest valid and unchanged for a moment. The rest is the first future each
  /// render asks for, so lurq keeps its state across renders and following it once follows every rest.
  /// No rest runs while a write is `writing`: what was edited meanwhile rests once the write ends, so it is
  /// saved after it rather than skipped.
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
  fn header(&self, ctx: &mut Ctx, props: &EditorProps, action: &Action, busy: bool) -> Element {
    let editing = self.state.get();
    let nav = Navigation::of(ctx);
    let id = props.project.open.project.id();
    let back = ActionButton::new("pipeline-parent", ButtonKind::Ghost, ctx.t("pipelines.title"))
      .on_click(move || nav.go(Route::Project(id, ProjectPage::Pipelines)));
    let status = save_status(ctx, &editing, busy);
    let publish = self.publish_button(ctx, &editing, busy);
    let mut head = parts::header()
      .child(back)
      .child(crate::ui::icon::icon(
        crate::ui::icon::Icon::ChevronRight,
        crate::theme::IconSize::Sm,
        lurq::app::theme::PaletteColor::TextMuted,
      ))
      .child(
        lurq::components::Text::new(&editing.opened.name)
          .id("pipeline-title")
          .variant(crate::theme::ExtraTypography::Strong.role()),
      )
      .with_children((editing.panel == Panel::Settings).then(|| {
        Row::new()
          .spacing(lurq::app::theme::SpacingSize::Sm)
          .child(crate::ui::icon::icon(
            crate::ui::icon::Icon::ChevronRight,
            crate::theme::IconSize::Sm,
            lurq::app::theme::PaletteColor::TextMuted,
          ))
          .child(
            lurq::components::Text::new(&ctx.t("pipelines.settings"))
              .variant(crate::theme::ExtraTypography::Strong.role()),
          )
      }))
      .child(crate::ui::icon::icon(
        crate::ui::icon::Icon::Check,
        crate::theme::IconSize::Sm,
        lurq::app::theme::PaletteColor::TextMuted,
      ))
      .child(parts::caption(&status))
      .child(Spacer::new().flex(1.0));
    if editing.edit.steps().is_empty() {
      head = head.child(parts::caption(&ctx.t("pipelines.publishEmpty")));
    }
    head = head.child(super::publication::run_button(ctx, &editing));
    if editing.viewing.is_some() {
      return head.child(publish.disabled()).into();
    }
    if editing.dirty() {
      let (state, asking, action, root) = (
        self.state.clone(),
        self.confirm_save.clone(),
        action.clone(),
        props.project.open.repo_root.clone(),
      );
      let save =
        ActionButton::new("pipeline-save", ButtonKind::Secondary, ctx.t("pipelines.save")).on_click(move || {
          let current = state.get();
          let Ok(body) = current.edit.validate() else {
            return;
          };
          if !draft_reasons(&current.opened, &body).is_empty() {
            asking.set(true);
            return;
          }
          action.run(Work::Save {
            root: root.clone(),
            name: current.name,
            before: current.opened,
            body,
            allowed: Allowed::default(),
          });
        });
      head = head.child(
        if busy || !editing.errors.is_empty() || editing.edit.validate().is_err() {
          save.disabled()
        } else {
          save
        },
      );
    }
    head.child(publish).into()
  }
  fn publish_button(&self, ctx: &Ctx, editing: &Editing, busy: bool) -> ActionButton {
    let panels = editing.panels.clone();
    if editing.opened.latest.is_some() && editing.opened.draft_saved_at.is_none() && !editing.dirty() {
      return ActionButton::new(
        "pipeline-header-history",
        ButtonKind::Secondary,
        ctx.t("pipelines.history"),
      )
      .icon(crate::ui::icon::Icon::History)
      .on_click(move || panels.choose(Panel::History));
    }
    let next = next_version(&editing.opened)
      .map(|v| v.get().to_string())
      .unwrap_or_default();
    let publish = ActionButton::new(
      "pipeline-publish",
      ButtonKind::Primary,
      ctx.t_args("pipelines.publish", [("version", next)]),
    )
    .icon(crate::ui::icon::Icon::ArrowUp)
    .on_click(move || panels.choose(Panel::Publish));
    if busy
      || editing.panel == Panel::Publish
      || editing.dirty()
      || !editing.errors.is_empty()
      || editing.opened.draft_saved_at.is_none()
      || !super::publication::valid(editing)
    {
      publish.disabled()
    } else {
      publish
    }
  }
  fn save_question(&self, ctx: &mut Ctx, props: &EditorProps, action: &Action) -> Element {
    let editing = self.state.get();
    let Ok(body) = editing.edit.validate() else {
      return field_error("pipeline-invalid", &ctx.t("pipelines.error")).into();
    };
    let reasons = draft_reasons(&editing.opened, &body);
    let words = reasons.iter().map(|m| message_text(ctx, m)).collect();
    let (state, done, keep, action, root) = (
      self.state.clone(),
      self.confirm_save.clone(),
      self.confirm_save.clone(),
      action.clone(),
      props.project.open.repo_root.clone(),
    );
    ctx.mount_keyed::<WidenConfirm>(
      "pipeline-save-question",
      WidenProps {
        id: "pipeline-save-question".into(),
        card: false,
        automation: crate::mcp::input_gate::Guarded::PermissionWidening,
        title: ctx.t("pipelines.saveWeaker"),
        reasons: words,
        confirm: ctx.t("pipelines.save"),
        keep: ctx.t("pipelines.keep"),
        on_confirm: Arc::new(move || {
          let current = state.get();
          if current.edit.validate().as_ref() != Ok(&body) || !current.errors.is_empty() {
            return;
          }
          done.set(false);
          action.run(Work::Save {
            root: root.clone(),
            name: current.name,
            before: current.opened,
            body: body.clone(),
            allowed: Allowed(reasons.clone()),
          });
        }),
        on_keep: Arc::new(move || keep.set(false)),
      },
    )
  }
  fn publish_panel(&self, ctx: &mut Ctx, props: &EditorProps, action: &Action, busy: bool) -> Element {
    let editing = self.state.get();
    let (body, valid) = super::publication::review(ctx, &editing);
    let version = next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string());
    let reasons = publish_reasons(editing.opened.latest.as_ref().map(|v| &v.body), &editing.opened.body);
    let footer = if valid && !busy {
      let (state, action, root, before) = (
        self.state.clone(),
        action.clone(),
        props.project.open.repo_root.clone(),
        editing.opened.clone(),
      );
      let keep = editing.panels.clone();
      if reasons.is_empty() {
        Some(super::publication::footer(
          ctx,
          &self.state,
          ActionButton::new(
            "pipeline-publish-confirm",
            ButtonKind::Primary,
            ctx.t_args("pipelines.publish", [("version", version.clone())]),
          )
          .icon(crate::ui::icon::Icon::ArrowUp)
          .on_click(move || {
            action.run(Work::Publish {
              root: root.clone(),
              before: before.clone(),
              allowed: Allowed::default(),
            });
          }),
        ))
      } else {
        let words = reasons.iter().map(|m| message_text(ctx, m)).collect();
        Some(ctx.mount_keyed::<WidenConfirm>(
          "pipeline-publish-question",
          WidenProps {
            id: "pipeline-publish-question".into(),
            card: false,
            automation: crate::mcp::input_gate::Guarded::PermissionWidening,
            title: ctx.t("pipelines.publishWeaker"),
            reasons: words,
            confirm: ctx.t("pipelines.publishNow"),
            keep: ctx.t("pipelines.keep"),
            on_confirm: Arc::new(move || {
              let current = state.get();
              if current.opened != before || current.dirty() || !current.errors.is_empty() {
                return;
              }
              action.run(Work::Publish {
                root: root.clone(),
                before: before.clone(),
                allowed: Allowed(reasons.clone()),
              });
            }),
            on_keep: Arc::new(move || keep.close()),
          },
        ))
      }
    } else {
      None
    };
    parts::panel(
      ctx,
      &ctx.t("pipelines.publishTitle"),
      &ctx.t_args(
        "pipelines.publishName",
        [("name", editing.opened.name.to_string()), ("version", version)],
      ),
      body.into(),
      footer,
      Some(self.state.clone()),
    )
  }
}
/// Takes in what a fulfilled write stored, once per editor: a failed write keeps the data of the one before
/// it.
fn follow_writes(ctx: &mut Ctx, action: &Action, state: &State) {
  let state = state.clone();
  ctx.watch(&action.state(), move |result| {
    if !result.is_pending()
      && result.error.is_none()
      && let Some(opened) = &result.data
    {
      state.update(|state| state.saved(opened.clone()));
      let editing = state.get_untracked();
      if editing.panel == Panel::Publish {
        editing.panels.close();
      }
    }
  });
}
fn save_status(ctx: &Ctx, editing: &Editing, busy: bool) -> std::sync::Arc<str> {
  if let Some(version) = editing.viewing_version {
    ctx.t_args("pipelines.viewing", [("version", version.get().to_string())])
  } else if busy {
    ctx.t("pipelines.saving")
  } else if editing.dirty() || !editing.errors.is_empty() {
    ctx.t("pipelines.unsaved")
  } else if let Some(latest) = &editing.opened.latest {
    ctx.t_args(
      if editing.opened.draft_saved_at.is_some() {
        "pipelines.savedPublished"
      } else {
        "pipelines.publishedHint"
      },
      [("version", latest.number.get().to_string())],
    )
  } else {
    ctx.t("pipelines.savedUnpublished")
  }
}
