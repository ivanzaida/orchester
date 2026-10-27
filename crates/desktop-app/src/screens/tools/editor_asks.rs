//! What a tool's page asks and offers in place: the draft bar's Discard draft, Publish and Create draft, the widening
//! question at the field and before publishing (which only the person answers), a draft older than the latest
//! version (*Keep this draft* or *Start from vN*), a change made elsewhere with Reload, and Discard draft and Remove.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::resource::{ResourceKind, Tool};
use orchester_workspace::{
  agents::data::VersionState,
  connections::mcp::Allowed,
  messages::Message,
  tasks::outside::Outside,
  tools::{
    data::ToolEntry,
    editor::{Edit, next_version},
    form::{CommandErrors, CommandText},
  },
  workspace::OpenProject,
};

use super::{
  editor::{AllowedHere, Ask, Asking, EditorProps, Sent, ToolEditor},
  work::{Start, Work},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::{
    connections::widen::{WidenConfirm, WidenProps},
    hub::{discard, removal},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    inspector::note,
    message::message_text,
    notice::{Notice, NoticeTone},
  },
};

/// Starts a write from a press, off the page's borrow.
#[derive(Clone)]
pub(super) struct Starter {
  action: Work,
  busy: Signal<bool>,
  sent: Signal<Sent>,
  restart: Signal<bool>,
  open: OpenProject,
}
impl Starter {
  pub(super) fn run(&self, edit: Edit, reasons: Vec<Message>) {
    if self.busy.get_untracked() {
      return;
    }
    if let Edit::Save { tool, .. } = &edit {
      self.sent.set(Sent(Some((**tool).clone())));
    }
    self.restart.set(matches!(edit, Edit::StartLatest(_)));
    self.busy.set(true);
    self.action.run(Start {
      open: self.open.clone(),
      edit,
      allowed: Allowed::These(reasons),
    });
  }
}

fn allow(allowed: &Signal<AllowedHere>, reasons: &[Message]) {
  allowed.update(|allowed| {
    for reason in reasons {
      if !allowed.0.contains(reason) {
        allowed.0.push(reason.clone());
      }
    }
  });
}

impl ToolEditor {
  pub(super) fn starter(&self, p: &EditorProps) -> Starter {
    Starter {
      action: self.action.clone(),
      busy: self.busy.clone(),
      sent: self.sent.clone(),
      restart: self.restart.clone(),
      open: p.project.open.clone(),
    }
  }

  /// Discard draft and Publish on a tool; Create draft on a new one.
  pub(super) fn bar_actions(
    &self,
    ctx: &Ctx,
    p: &EditorProps,
    (checked, checking): (&Result<Edit, CommandErrors>, bool),
    (unchanged, outside): (bool, bool),
  ) -> Vec<Element> {
    let busy = self.busy.get();
    let Some(entry) = self.baseline.get().0 else {
      let create = ActionButton::new("tool-create", ButtonKind::Primary, ctx.t("tools.create")).icon(Icon::Plus);
      let edit = checked.as_ref().ok().filter(|_| !checking).cloned();
      return vec![self.action_button(p, create, edit).into()];
    };
    let mut actions = Vec::new();
    if entry.state.has_draft() && entry.state.published().is_some() {
      let ask = self.ask.clone();
      let discard = ActionButton::new("tool-discard", ButtonKind::Ghost, ctx.t("hub.discard.action")).icon(Icon::Undo2);
      actions.push(if busy || !unchanged || outside {
        discard.disabled().into()
      } else {
        discard.on_click(move || ask.set(Ask::Discard)).into()
      });
    }
    let label = next_version(Some(&entry)).map_or(ctx.t("tools.versionLimit"), |v| {
      ctx.t_args("tools.publish", [("version", v.get().to_string())])
    });
    let publish = ActionButton::new("tool-publish", ButtonKind::Primary, label).icon(Icon::Upload);
    let edit =
      (publish_reason(ctx, Some(&entry), unchanged).is_none() && !outside).then(|| Edit::Publish(entry.clone()));
    let button = self.action_button(p, publish, edit);
    actions.push(match publish_reason(ctx, Some(&entry), unchanged) {
      Some(reason) if !busy => button.unavailable_with_icon(reason).into(),
      _ => button.into(),
    });
    actions
  }

  /// `button` runs `edit`, asking first for what it widens that the person did not allow here.
  pub(super) fn action_button(&self, p: &EditorProps, button: ActionButton, edit: Option<Edit>) -> ActionButton {
    if self.busy.get() {
      return button.loading();
    }
    let Some(edit) = edit.filter(|_| self.asking.get().0.is_none()) else {
      return button.disabled();
    };
    let reasons = edit.reasons(p.data.latest.as_ref());
    let unapproved = reasons.iter().any(|reason| !self.allowed.get().0.contains(reason));
    let (asking, starter) = (self.asking.clone(), self.starter(p));
    button.on_click(move || {
      if unapproved {
        asking.set(Asking(Some(edit.clone())));
      } else {
        starter.run(edit.clone(), reasons.clone());
      }
    })
  }

  /// Publishing or creating what widens the tool, asked in place of the draft bar.
  pub(super) fn confirm(&self, ctx: &mut Ctx, p: &EditorProps, edit: Edit) -> Element {
    let reasons = edit.reasons(p.data.latest.as_ref());
    self.question(ctx, "tool-confirm", (edit, reasons), self.asking.clone())
  }

  /// A saved change that widens the tool, asked at its field before it is saved.
  pub(super) fn widening(&self, ctx: &mut Ctx, edit: Edit, reasons: Vec<Message>) -> Element {
    self.question(ctx, "tool-widen", (edit, reasons), self.asking.clone())
  }

  fn question(
    &self,
    ctx: &mut Ctx,
    id: &str,
    (edit, reasons): (Edit, Vec<Message>),
    asking: Signal<Asking>,
  ) -> Element {
    let mut words: Vec<Arc<str>> = reasons.iter().map(|r| message_text(ctx, r)).collect();
    words.push(ctx.t("tools.execution"));
    let starter = self.starter(&ctx.props::<EditorProps>().clone());
    let (allowed, keep, sent) = (self.allowed.clone(), asking.clone(), self.sent.clone());
    let (fields, text, name) = (self.fields.clone(), self.fields.text(), self.fields.name.get());
    let declined = match &edit {
      Edit::Save { tool, .. } => Some((**tool).clone()),
      _ => None,
    };
    ctx.mount_keyed::<WidenConfirm>(
      id,
      WidenProps {
        id: id.into(),
        card: true,
        affirming: false,
        automation: crate::mcp::input_gate::Guarded::PermissionWidening,
        title: ctx.t("tools.confirmTitle"),
        reasons: words.into(),
        confirm: ctx.t("tools.confirm"),
        keep: ctx.t("tools.keepEditing"),
        on_confirm: Arc::new(move || {
          if matches!(edit, Edit::Save { .. }) && (fields.text() != text || fields.name.get_untracked() != name) {
            return;
          }
          allow(&allowed, &reasons);
          asking.set(Asking(None));
          starter.run(edit.clone(), reasons.clone());
        }),
        on_keep: Arc::new(move || {
          keep.set(Asking(None));
          // The change is not saved until the person edits it again.
          if let Some(tool) = &declined {
            sent.set(Sent(Some(tool.clone())));
          }
        }),
      },
    )
  }

  /// A draft older than the latest version: keep it as it is, or start again from the latest version.
  pub(super) fn stale(&self, ctx: &mut Ctx, p: &EditorProps, locked: bool) -> Option<Element> {
    let (entry, VersionState::StaleDraft { based_on, published }) = self.stale_state()? else {
      return None;
    };
    let version = published.map_or(0, |v| v.get()).to_string();
    let start = ActionButton::new(
      "tool-start-latest",
      ButtonKind::Secondary,
      ctx.t_args("tools.startLatest", [("version", version.clone())]),
    );
    let keep = ActionButton::new("tool-keep-draft", ButtonKind::Primary, ctx.t("tools.keepDraft"));
    let same = CommandText::of(&entry.tool).is_some_and(|text| text == self.fields.text());
    let enabled = !locked && same && Some(&entry) == p.entry.as_ref();
    let start = self.action_button(p, start, enabled.then(|| Edit::StartLatest(entry.clone())));
    let keep = self.action_button(p, keep, enabled.then(|| Edit::KeepDraft(entry.clone())));
    let baseline = based_on.map_or(ctx.t("tools.noBaseline").to_string(), |v| {
      ctx
        .t_args("tools.compactVersion", [("version", v.get().to_string())])
        .to_string()
    });
    Some(
      Notice::new("tool-stale", NoticeTone::Warning, ctx.t("tools.staleTitle"))
        .body(ctx.t_args("tools.staleDetail", [("baseline", baseline), ("version", version)]))
        .action(start)
        .action(keep)
        .into(),
    )
  }

  /// The tool changed elsewhere while this page showed it: Reload starts over from what is saved.
  pub(super) fn outside_change(&self, ctx: &Ctx, p: &EditorProps) -> Option<Element> {
    if self.baseline.get().0 == p.entry {
      return None;
    }
    let reload = p.reload.clone();
    Some(
      Notice::new("tool-changed", NoticeTone::Warning, ctx.t("tools.changed"))
        .action(
          ActionButton::new("tool-reload", ButtonKind::Secondary, ctx.t("tools.reload"))
            .icon(Icon::RotateCcw)
            .on_click(move || reload.run()),
        )
        .into(),
    )
  }

  /// Discard draft, asked in place of the draft bar.
  pub(super) fn discard_question(&self, ctx: &mut Ctx, p: &EditorProps, entry: Option<&ToolEntry>) -> Option<Element> {
    let entry = entry.filter(|_| self.ask.get() == Ask::Discard)?;
    let published = entry.state.published().map_or(0, |v| v.get());
    let (root, id, expected) = (p.project.open.repo_root.clone(), entry.id, entry.tool.clone());
    let (keep, done, reload) = (self.ask.clone(), self.ask.clone(), p.reload.clone());
    Some(discard::question(
      ctx,
      discard::DiscardProps {
        project: p.project.clone(),
        versions: (published + 1, published),
        discard: Arc::new(move || orchester_workspace::removal::discard_in::<Tool>(&root, id, &expected)),
        change: Outside::Tools,
        on_keep: Arc::new(move || keep.set(Ask::Nothing)),
        on_discarded: Arc::new(move || {
          done.set(Ask::Nothing);
          reload.run();
        }),
      },
    ))
  }

  /// Remove, asked under the head; the list opens once it is removed.
  pub(super) fn remove_question(&self, ctx: &mut Ctx, p: &EditorProps, entry: Option<&ToolEntry>) -> Option<Element> {
    let entry = entry.filter(|_| self.ask.get() == Ask::Remove)?;
    let (keep, navigation, project) = (self.ask.clone(), Navigation::of(ctx), p.project.open.project.id());
    Some(removal::question(
      ctx,
      removal::RemoveProps {
        project: p.project.clone(),
        resource: entry.id,
        kind: ResourceKind::Tool,
        name: entry.name.clone(),
        on_keep: Arc::new(move || keep.set(Ask::Nothing)),
        on_removed: Arc::new(move || navigation.replace(Route::Project(project, ProjectPage::Tools))),
      },
    ))
  }
}

/// A tool this version cannot edit, said in place of its fields.
pub(super) fn read_only(ctx: &Ctx, p: &EditorProps) -> Element {
  lurq::components::Column::new()
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Lg)
    .child(
      crate::ui::state::EmptyState::new(
        "tool-read-only",
        Icon::Lock,
        ctx.t("tools.readOnlyTitle"),
        ctx.t(match p.entry.as_ref().map(|e| e.tool.action()) {
          Some(orchester_core::resource::ToolAction::Http(_)) => "tools.httpUnavailable",
          _ => "tools.databaseUnavailable",
        }),
      )
      .wide(),
    )
    .child(note("tool-command-only", Icon::Info, &ctx.t("tools.readOnlyDetail")))
    .into()
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
