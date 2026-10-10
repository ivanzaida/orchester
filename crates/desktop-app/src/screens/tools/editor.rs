//! Command editor lifecycle and in-place questions, no modal (`S5Gn5R`, `QyM60`).
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, ScrollVertical},
  core::Signal,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  agents::data::VersionState,
  connections::mcp::Allowed,
  tools::{
    data::ToolEntry,
    editor::{Edit, EditorData, next_version},
    form::{CommandErrors, CommandText},
  },
};

use super::{
  fields::Fields,
  head, inspector, words,
  work::{self, Start, Work},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, ToolTab},
  screens::connections::widen::{WidenConfirm, WidenProps},
  theme::{ConnectionsSpacing, ExtraSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::note,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

#[derive(Clone)]
pub struct EditorProps {
  pub project: ShownProject,
  pub data: EditorData,
  pub entry: Option<ToolEntry>,
  pub revision: Signal<u64>,
  pub page: ProjectPage,
}
impl PartialEq for EditorProps {
  fn eq(&self, o: &Self) -> bool {
    self.project == o.project && self.data == o.data && self.entry == o.entry && self.page == o.page
  }
}
impl DevtoolsInspectable for EditorProps {}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Asking(Option<Edit>);
impl DevtoolsInspectable for Asking {}
pub struct ToolEditor {
  fields: Fields,
  baseline: Option<ToolEntry>,
  text: CommandText,
  asking: Signal<Asking>,
  busy: Signal<bool>,
}
impl Component for ToolEditor {
  type Props = EditorProps;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let text = props
      .entry
      .as_ref()
      .and_then(|e| CommandText::of(&e.tool))
      .unwrap_or_else(CommandText::new_tool);
    let name = props.entry.as_ref().map_or("", |e| &*e.name);
    let editor = Self {
      fields: Fields::new(ctx, name, &text),
      baseline: props.entry.clone(),
      text,
      asking: ctx.signal(Asking(None)),
      busy: ctx.signal(false),
    };
    let action = work::action(ctx);
    let (revision, busy) = (props.revision.clone(), editor.busy.clone());
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    ctx.watch(&action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      busy.set(false);
      if let Some(saved) = state.data.filter(|_| state.is_fulfilled()) {
        revision.update(|r| *r += 1);
        let tab = match navigation.route_now() {
          Route::Project(_, ProjectPage::Tool(_, tab)) => tab,
          _ => ToolTab::Configuration,
        };
        navigation.replace(Route::Project(project, ProjectPage::Tool(saved.0, tab)));
      }
    });
    editor
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<EditorProps>().clone();
    let action = work::action(ctx);
    let (checked, checking) = super::validation::checked(ctx, &self.fields, self.baseline.as_ref(), &p);
    let unchanged = self.fields.text() == self.text && self.baseline.is_some();
    let changed = self.baseline != p.entry;
    let busy = self.busy.get();
    let state = if busy {
      ctx.t("tools.saving")
    } else if self.baseline.is_none() {
      ctx.t("tools.notSaved")
    } else if !unchanged {
      ctx.t("tools.unsaved")
    } else {
      match self.baseline.as_ref().expect("the baseline exists").state {
        VersionState::Draft { .. } => ctx.t("tools.draftSaved"),
        _ => words::version(ctx, self.baseline.as_ref().expect("the baseline exists")),
      }
    };
    let title = self
      .baseline
      .as_ref()
      .map_or(ctx.t("tools.newTitle"), |e| e.name.clone());
    let errors = checked.as_ref().err().cloned().unwrap_or_default();
    let read_only = self
      .baseline
      .as_ref()
      .is_some_and(|e| CommandText::of(&e.tool).is_none());
    let save_reason = if read_only {
      Some(ctx.t("tools.readOnlyTitle"))
    } else if checking {
      Some(ctx.t("tools.checking"))
    } else if changed {
      Some(ctx.t("tools.changed"))
    } else if checked.is_err() {
      Some(ctx.t("tools.fixFields"))
    } else {
      None
    };
    let publish_reason = publish_reason(ctx, self.baseline.as_ref(), unchanged);
    let save = checked.ok().filter(|_| !checking && !unchanged && !changed && !busy);
    let publish = self
      .baseline
      .clone()
      .filter(|_| publish_reason.is_none() && !changed && !busy)
      .map(Edit::Publish);
    let buttons = vec![
      self.save_button(ctx, save, &p, &action, save_reason.as_deref()).into(),
      self
        .publish_button(ctx, publish, &p, &action, publish_reason.as_deref())
        .into(),
    ];
    let reason = action
      .state()
      .get()
      .error
      .as_ref()
      .map(|e| message_text(ctx, e))
      .or(save_reason)
      .or(publish_reason);
    let head = head::head(ctx, &p, &title, &state, buttons, reason.as_deref());
    let body = self.body(ctx, &p, &errors, busy || self.asking.get().0.is_some(), &action);
    let question = self.asking.get().0.map(|edit| self.confirm(ctx, &p, &action, edit));
    Column::new()
      .id("tool-editor")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(head)
      .with_children(question)
      .child(body)
  }
}

impl ToolEditor {
  fn save_button(
    &self,
    ctx: &Ctx,
    edit: Option<Edit>,
    p: &EditorProps,
    action: &Work,
    reason: Option<&str>,
  ) -> ActionButton {
    let button = ActionButton::new("tool-save", ButtonKind::Secondary, ctx.t("tools.save")).icon(Icon::Save);
    self.action_button(button, edit, p, action, reason)
  }
  fn publish_button(
    &self,
    ctx: &Ctx,
    edit: Option<Edit>,
    p: &EditorProps,
    action: &Work,
    reason: Option<&str>,
  ) -> ActionButton {
    let label = next_version(self.baseline.as_ref()).map_or(ctx.t("tools.versionLimit"), |v| {
      ctx.t_args("tools.publish", [("version", v.get().to_string())])
    });
    self.action_button(
      ActionButton::new("tool-publish", ButtonKind::Primary, label).icon(Icon::Upload),
      edit,
      p,
      action,
      reason,
    )
  }
  fn action_button(
    &self,
    button: ActionButton,
    edit: Option<Edit>,
    p: &EditorProps,
    action: &Work,
    reason: Option<&str>,
  ) -> ActionButton {
    if self.busy.get() {
      return button.loading();
    }
    if self.asking.get().0.is_some() {
      return button.disabled();
    }
    let Some(edit) = edit else {
      return match reason {
        Some(r) => button.unavailable(r.to_owned()),
        None => button.disabled(),
      };
    };
    let reasons = edit.reasons(p.data.latest.as_ref());
    let (asking, busy, action, open) = (
      self.asking.clone(),
      self.busy.clone(),
      action.clone(),
      p.project.open.clone(),
    );
    button.on_click(move || {
      if busy.get_untracked() {
        return;
      }
      if !reasons.is_empty() {
        asking.set(Asking(Some(edit.clone())));
      } else {
        busy.set(true);
        action.run(Start {
          open: open.clone(),
          edit: edit.clone(),
          allowed: Allowed::These(Vec::new()),
        });
      }
    })
  }
  fn confirm(&self, ctx: &mut Ctx, p: &EditorProps, action: &Work, edit: Edit) -> Element {
    let reasons = edit.reasons(p.data.latest.as_ref());
    let mut words: Vec<Arc<str>> = reasons.iter().map(|r| message_text(ctx, r)).collect();
    words.push(ctx.t("tools.execution"));
    let (asking, keep, busy, action) = (
      self.asking.clone(),
      self.asking.clone(),
      self.busy.clone(),
      action.clone(),
    );
    let (open, fields, text, name) = (
      p.project.open.clone(),
      self.fields.clone(),
      self.fields.text(),
      self.fields.name.get(),
    );
    ctx.mount_keyed::<WidenConfirm>(
      "tool-confirm",
      WidenProps {
        id: "tool-confirm",
        title: ctx.t("tools.confirmTitle"),
        reasons: words.into(),
        confirm: ctx.t("tools.confirm"),
        keep: ctx.t("tools.keepEditing"),
        on_confirm: Arc::new(move || {
          if busy.get_untracked() || fields.text() != text || fields.name.get_untracked() != name {
            return;
          }
          busy.set(true);
          asking.set(Asking(None));
          action.run(Start {
            open: open.clone(),
            edit: edit.clone(),
            allowed: Allowed::These(reasons.clone()),
          });
        }),
        on_keep: Arc::new(move || keep.set(Asking(None))),
      },
    )
  }
  fn body(&self, ctx: &mut Ctx, p: &EditorProps, errors: &CommandErrors, locked: bool, action: &Work) -> Row {
    let tab = match p.page {
      ProjectPage::Tool(_, tab) => tab,
      _ => ToolTab::Configuration,
    };
    let read_only = p.entry.as_ref().is_some_and(|e| CommandText::of(&e.tool).is_none());
    let mut main = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .padding(
        Padding::new()
          .top(ExtraSpacing::Tabs.role())
          .bottom(SpacingSize::Lg)
          .left(ConnectionsSpacing::Page.role())
          .right(ConnectionsSpacing::Page.role()),
      );
    if self.baseline != p.entry {
      let revision = p.revision.clone();
      main = main.child(
        Notice::new("tool-changed", NoticeTone::Warning, ctx.t("tools.changed")).action(
          ActionButton::new("tool-reload", ButtonKind::Secondary, ctx.t("tools.reload"))
            .on_click(move || revision.update(|r| *r += 1)),
        ),
      );
    }
    let content: Element = if read_only {
      Notice::new("tool-read-only", NoticeTone::Info, ctx.t("tools.readOnlyTitle"))
        .body(ctx.t("tools.readOnlyDetail"))
        .into()
    } else {
      match tab {
        ToolTab::Configuration => {
          main = main.with_children(self.stale(ctx, p, locked, action));
          self.fields.render(ctx, errors, self.baseline.is_some(), locked).into()
        }
        ToolTab::Access => super::access::access(ctx, p).into(),
        ToolTab::InputsOutputs => Notice::new("tool-inputs-unavailable", NoticeTone::Info, ctx.t("tools.inputsTitle"))
          .body(ctx.t("tools.inputsDetail"))
          .into(),
        ToolTab::UsageLog => Notice::new("tool-usage-unavailable", NoticeTone::Info, ctx.t("tools.usageTitle"))
          .body(ctx.t("tools.usageDetail"))
          .into(),
      }
    };
    main = main.child(content);
    if !read_only {
      main = main.child(note("tool-execution", Icon::TriangleAlert, &ctx.t("tools.execution")));
    }
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .min_height(Dimension::Px(0.0))
      .child(ScrollVertical::new(main).id("tool-form-scroll").flex(1.0))
      .child(inspector::inspector(ctx, p))
  }
  fn stale(&self, ctx: &mut Ctx, p: &EditorProps, locked: bool, action: &Work) -> Option<Element> {
    let e = self.baseline.as_ref()?;
    let VersionState::StaleDraft { published, .. } = e.state else {
      return None;
    };
    let start = ActionButton::new(
      "tool-start-latest",
      ButtonKind::Secondary,
      ctx.t_args(
        "tools.startLatest",
        [("version", published.map_or(0, |v| v.get()).to_string())],
      ),
    );
    let keep = ActionButton::new("tool-keep-draft", ButtonKind::Primary, ctx.t("tools.keepDraft"));
    let enabled = !locked && self.fields.text() == self.text && self.baseline == p.entry;
    let start = self.action_button(start, enabled.then(|| Edit::StartLatest(e.clone())), p, action, None);
    let keep = self.action_button(keep, enabled.then(|| Edit::KeepDraft(e.clone())), p, action, None);
    Some(
      Notice::new("tool-stale", NoticeTone::Warning, ctx.t("tools.staleTitle"))
        .body(ctx.t("tools.staleDetail"))
        .action(start)
        .action(keep)
        .into(),
    )
  }
}
fn publish_reason(ctx: &Ctx, entry: Option<&ToolEntry>, unchanged: bool) -> Option<Arc<str>> {
  let Some(entry) = entry else {
    return Some(ctx.t("tools.enterRequired"));
  };
  if CommandText::of(&entry.tool).is_none() {
    return Some(ctx.t("tools.readOnlyTitle"));
  }
  if !unchanged {
    return Some(ctx.t("tools.saveFirst"));
  }
  if matches!(entry.state, VersionState::StaleDraft { .. }) {
    return Some(ctx.t("tools.staleFirst"));
  }
  if !entry.state.has_draft() {
    return Some(ctx.t_args(
      "tools.noChanges",
      [("version", entry.state.published().map_or(0, |v| v.get()).to_string())],
    ));
  }
  next_version(Some(entry)).is_none().then(|| ctx.t("tools.versionLimit"))
}
