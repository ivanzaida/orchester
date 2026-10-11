//! Loaded draft, graph and inspector, with explicit saves and an in-place publish review.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Row, Spacer},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ResourceId;
use orchester_workspace::{
  background,
  pipelines::{
    data::OpenedPipeline,
    problems::problem_view,
    widening::{draft_reasons, publish_reasons},
    write::{Allowed, next_version},
  },
};

use super::{
  parts,
  state::{Editing, Panel, State},
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
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
pub struct LoadProps {
  pub project: ShownProject,
  pub id: ResourceId,
}
impl DevtoolsInspectable for LoadProps {}
pub struct LoadEditor {
  retry: Signal<u64>,
}
impl Component for LoadEditor {
  type Props = LoadProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self { retry: ctx.signal(0) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LoadProps>().clone();
    let revision = self.retry.get();
    let loaded = ctx
      .future(
        (props.project.open.repo_root.clone(), props.id, revision),
        |(root, id, _)| async move {
          background::run(move || OpenedPipeline::load_in(&root, id).map_err(super::work_error)).await?
        },
      )
      .state()
      .get();
    if loaded.is_pending() {
      return parts::page()
        .child(loading("pipeline-loading", &ctx.t("pipelines.reading")))
        .into();
    }
    if let Some(opened) = loaded.data {
      return ctx.mount_keyed::<Editor>(
        &format!("pipeline-editor-{}", self.retry.get()),
        EditorProps {
          project: props.project,
          opened,
          reload: self.retry.clone(),
        },
      );
    }
    let content = match loaded.error {
      Some(error) if !loaded.is_pending() => {
        let retry = self.retry.clone();
        load_error(
          ctx,
          "pipeline-load-error",
          (&ctx.t("pipelines.loadFailed"), &message_text(ctx, &error)),
          move || retry.update(|v| *v += 1),
        )
      }
      _ => loading("pipeline-loading", &ctx.t("pipelines.reading")).into(),
    };
    parts::page().child(content).into()
  }
}
#[derive(Clone)]
pub struct EditorProps {
  pub project: ShownProject,
  pub opened: OpenedPipeline,
  pub reload: Signal<u64>,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.opened == other.opened
  }
}
impl DevtoolsInspectable for EditorProps {}
#[derive(Clone, PartialEq)]
struct Snapshot(Option<(orchester_core::resource::Pipeline, orchester_core::name::Name)>);
impl DevtoolsInspectable for Snapshot {}
pub struct Editor {
  state: State,
  confirm_save: Signal<bool>,
  writing: Signal<bool>,
  leave: crate::navigation::guard::LeaveGuard,
}
impl Component for Editor {
  type Props = EditorProps;
  fn create(ctx: &mut Ctx) -> Self {
    let state = ctx.signal(Editing::new(ctx.props::<EditorProps>().opened.clone()));
    let writing = ctx.signal(false);
    let (edits, pending) = (state.clone(), writing.clone());
    let leave = Navigation::of(ctx).guard(move || {
      let edits = edits.get_untracked();
      edits.dirty() || !edits.errors.is_empty() || pending.get_untracked()
    });
    Self {
      state,
      writing,
      leave,
      confirm_save: ctx.signal(false),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    let action = work::action(ctx, props.project.open.project.id());
    let writing = action.state().get().is_pending();
    if self.writing.get_untracked() != writing {
      self.writing.set(writing);
    }
    self.autosave(ctx, &props, &action);
    let opened = self.state.get().opened;
    props.project.feed.pipeline_named(opened.id, &opened.name);
    let state = self.state.clone();
    ctx.watch(&action.state(), move |result| {
      if !result.is_pending()
        && let Some(opened) = &result.data
      {
        state.update(|state| {
          state.saved(opened.clone());
          if state.panel == Panel::Publish {
            state.panel = Panel::Pipeline;
          }
        });
      }
    });
    let editing = self.state.get();
    let busy = action.state().get().is_pending() || self.confirm_save.get();
    let head = self.header(ctx, &props, &action, busy);
    let content = if editing.panel == Panel::Settings {
      super::settings::page(ctx, &self.state, self.confirm_save.get())
    } else {
      let graph = super::canvas::canvas(ctx, &self.state, busy);
      let panel = match &editing.panel {
        Panel::Publish => self.publish_panel(ctx, &props, &action, busy),
        Panel::History => ctx.mount_keyed::<super::history::History>(
          "pipeline-history",
          super::history::Props {
            project: props.project.clone(),
            state: self.state.clone(),
          },
        ),
        _ => super::inspector::panel(ctx, &self.state, self.confirm_save.get()),
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
    for (field, error) in &editing.errors {
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
    super::publication::with_success(ctx, &self.state, page.into())
  }
}
impl Editor {
  fn autosave(&self, ctx: &mut Ctx, props: &EditorProps, action: &Action) {
    let editing = self.state.get();
    let body = (editing.dirty() && editing.errors.is_empty() && editing.viewing.is_none() && !self.leave.requested())
      .then(|| editing.edit.validate().ok().map(|body| (body, editing.name.clone())))
      .flatten();
    let ready = ctx
      .future(body, |body| async move {
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        Ok::<_, orchester_workspace::messages::Message>(Snapshot(body))
      })
      .state();
    let (state, action, root) = (self.state.clone(), action.clone(), props.project.open.repo_root.clone());
    let leave = self.leave.clone();
    ctx.watch(&ready, move |ready| {
      let Some(Snapshot(Some((body, name)))) = &ready.data else {
        return;
      };
      let current = state.get();
      let writing = action.state().get();
      if ready.is_pending()
        || leave.requested()
        || writing.is_pending()
        || writing.error.is_some()
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
          .child(parts::caption(&ctx.t("pipelines.settings")))
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
    if editing.opened.latest.is_some() && editing.opened.draft_saved_at.is_none() && !editing.dirty() {
      let state = self.state.clone();
      return ActionButton::new(
        "pipeline-header-history",
        ButtonKind::Secondary,
        ctx.t("pipelines.history"),
      )
      .icon(crate::ui::icon::Icon::History)
      .on_click(move || state.update(|s| s.panel = Panel::History));
    }
    let next = next_version(&editing.opened)
      .map(|v| v.get().to_string())
      .unwrap_or_default();
    let state = self.state.clone();
    let publish = ActionButton::new(
      "pipeline-publish",
      ButtonKind::Primary,
      ctx.t_args("pipelines.publish", [("version", next)]),
    )
    .on_click(move || state.update(|s| s.panel = Panel::Publish));
    if busy
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
        id: "pipeline-save-question",
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
    let reasons = publish_reasons(editing.opened.latest.as_ref().map(|v| &v.body), &editing.opened.body);
    let footer = if valid && !busy {
      let (state, action, root, before) = (
        self.state.clone(),
        action.clone(),
        props.project.open.repo_root.clone(),
        editing.opened.clone(),
      );
      let keep = self.state.clone();
      if reasons.is_empty() {
        Some(super::publication::footer(
          ctx,
          &self.state,
          ActionButton::new(
            "pipeline-publish-confirm",
            ButtonKind::Primary,
            ctx.t_args(
              "pipelines.publish",
              [(
                "version",
                next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string()),
              )],
            ),
          )
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
            id: "pipeline-publish-question",
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
            on_keep: Arc::new(move || keep.update(|s| s.panel = Panel::Pipeline)),
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
        [
          ("name", editing.opened.name.to_string()),
          (
            "version",
            next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string()),
          ),
        ],
      ),
      body.into(),
      footer,
      Some(self.state.clone()),
    )
  }
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
