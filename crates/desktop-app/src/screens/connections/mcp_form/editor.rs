//! The MCP server's fields, filled once from what the page read (the stored server, or nothing for a new one),
//! the head, and the action bar. A field whose value cannot be used says why under it; the action bar says the
//! first reason in field order. A save that widens what agents may do (`connections::mcp::widenings`) asks a
//! person in place first (`widen`); one that only narrows is saved at once. Confirming saves the fields as they are
//! when it is pressed, only while they widen what the question says; the save itself then checks that against the
//! server as stored (`Allowed`), which automation may have changed meanwhile.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
    theme::PaletteColor,
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  connection::mcp::{McpServer, McpServerName},
  id::ConnectionId,
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::ProjectConnections,
    mcp::{Allowed, McpServerDraft, McpServerErrors, McpServerText, store_in, widenings},
  },
  messages::Message,
  tasks::outside::Outside,
};

use super::fields::Fields;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ConnectionsTab, ConnectionsView, ProjectPage, Route, Screen},
  screens::connections::{
    data::Saved,
    page::{Head, action_bar, page, question_bar},
    widen::{WidenConfirm, WidenProps},
  },
  services::Services,
  ui::{
    breadcrumb::Crumb,
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub struct EditorProps {
  pub project: ShownProject,
  /// The server being edited; `None` adds one.
  pub stored: Option<McpServer>,
  /// The fields the editor opens with.
  pub text: McpServerText,
  pub listed: ProjectConnections,
}

impl DevtoolsInspectable for EditorProps {}

/// What a save stores, and what it may widen.
#[derive(Clone)]
struct Save {
  repo_root: PathBuf,
  id: Option<ConnectionId>,
  draft: McpServerDraft,
  allowed: Allowed,
}

type SaveAction = FutureAction<Save, Saved, Message>;

pub struct McpEditor {
  save: SaveAction,
  fields: Fields,
  /// A save was asked for: every field's error shows, also of fields left empty.
  tried: Signal<bool>,
  /// A save that widens waits for a person's answer.
  confirming: Signal<bool>,
}

impl Component for McpEditor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let text = ctx.props::<EditorProps>().text.clone();
    let editor = Self {
      save: save_action(ctx),
      fields: Fields::new(ctx, &text),
      tried: ctx.signal(false),
      confirming: ctx.signal(false),
    };
    let save = editor.save.clone();
    let project = ctx.props::<EditorProps>().project.open.project.id();
    let navigation = Navigation::of(ctx);
    let changes = Services::of(ctx).changes;
    ctx.watch(&save.state(), move |state| {
      if let Some(Saved(id)) = state.data.filter(|_| state.is_fulfilled()) {
        let view = ConnectionsView::new(ConnectionsTab::McpServers).with_selected(id);
        navigation.replace(Route::Project(project, ProjectPage::Connections(view)));
        // After the form leaves: every screen that shows the project's connections reads them again, the
        // status bar's accounts too.
        changes.record(project, Outside::Connections);
      }
    });
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    let save = self.save.clone();
    let screen = if props.stored.is_some() {
      Screen::EditConnection
    } else {
      Screen::NewConnection
    };
    let editing = props.stored.as_ref().map(McpServer::id);
    let text = self.fields.text();
    let checked = text.draft(&taken(&props.listed, editing), &props.listed.secrets);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let unchanged = text == props.text && props.stored.is_some();
    let sections = self.fields.sections(
      ctx,
      (&props.listed, &errors),
      self.tried.get(),
      props.project.open.project.name().as_str(),
    );
    let reason = reason(ctx, (&checked, &errors), unchanged, &save.state().get());
    let bar = self.bar(ctx, &props, (&save, checked.ok()), reason.as_deref());
    let head = head(ctx, &props, unchanged, &save.state().get());
    page(ctx, screen, head, sections, bar)
  }
}

impl McpEditor {
  fn bar(
    &self,
    ctx: &mut Ctx,
    props: &EditorProps,
    (save, draft): (&SaveAction, Option<McpServerDraft>),
    reason: Option<&str>,
  ) -> Element {
    let start = draft.filter(|_| reason.is_none()).map(|draft| Save {
      repo_root: props.project.open.repo_root.clone(),
      id: props.stored.as_ref().map(McpServer::id),
      draft,
      allowed: Allowed::These(Vec::new()),
    });
    if self.confirming.get()
      && let Some(start) = &start
    {
      return self.confirm(ctx, props, save, start);
    }
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    let back = ConnectionsView::new(ConnectionsTab::McpServers);
    let back = match props.stored.as_ref() {
      Some(server) => back.with_selected(server.id()),
      None => back,
    };
    let cancel = ActionButton::new("connection-cancel", ButtonKind::Secondary, ctx.t("connections.cancel"))
      .on_click(move || navigation.go(Route::Project(project, ProjectPage::Connections(back))));
    let button = ActionButton::new("connection-save", ButtonKind::Primary, ctx.t("connections.dialog.save"));
    let tried = self.tried.clone();
    let button = match (save.state().get().is_pending(), start) {
      (true, _) => button.loading(),
      (false, Some(start)) => {
        let widens = !widenings(props.stored.as_ref(), &start.draft).is_empty();
        let (save, confirming) = (save.clone(), self.confirming.clone());
        button.on_click(move || {
          if widens {
            confirming.set(true);
          } else {
            save.run(start.clone());
          }
        })
      }
      (false, None) if !self.tried.get() => button.on_click(move || tried.set(true)),
      (false, None) => button.disabled(),
    };
    action_bar(vec![cancel.into(), button.into()], reason)
  }

  /// What saving widens, for a person to confirm in place of the actions. The question's handlers outlive renders
  /// that only change a value (its props compare by their words), so confirming reads the fields then.
  fn confirm(&self, ctx: &mut Ctx, props: &EditorProps, save: &SaveAction, start: &Save) -> Element {
    let shown = widenings(props.stored.as_ref(), &start.draft);
    let reasons: Arc<[Arc<str>]> = shown.iter().map(|reason| message_text(ctx, reason)).collect();
    let (save, done, keep) = (save.clone(), self.confirming.clone(), self.confirming.clone());
    let (fields, start) = (self.fields.clone(), start.clone());
    let (listed, stored) = (props.listed.clone(), props.stored.clone());
    let question = WidenProps {
      card: false,
      automation: crate::mcp::input_gate::Guarded::PermissionWidening,
      id: "connection-confirm-save".into(),
      title: ctx.t("connections.mcp.dialog.widenTitle"),
      reasons,
      confirm: ctx.t("connections.dialog.save"),
      keep: ctx.t("connections.mcp.dialog.keepEditing"),
      on_confirm: Arc::new(move || {
        let editing = stored.as_ref().map(McpServer::id);
        // Fields that no longer make a server, or that widen what the question does not say yet, are not saved:
        // the next render shows their reason, or the question with their reasons.
        let Ok(draft) = fields.text().draft(&taken(&listed, editing), &listed.secrets) else {
          return;
        };
        if widenings(stored.as_ref(), &draft) != shown {
          return;
        }
        done.set(false);
        save.run(Save {
          draft,
          allowed: Allowed::These(shown.clone()),
          ..start.clone()
        });
      }),
      on_keep: Arc::new(move || keep.set(false)),
    };
    question_bar(ctx.mount_keyed::<WidenConfirm>("connection-confirm-save", question))
  }
}

/// The names of the project's other servers.
fn taken(listed: &ProjectConnections, editing: Option<ConnectionId>) -> Vec<McpServerName> {
  listed
    .mcp
    .iter()
    .filter(|server| Some(server.id()) != editing)
    .map(|server| server.name().clone())
    .collect()
}

/// The first reason Save cannot be used, in field order; `None` once it can.
fn reason(
  ctx: &Ctx,
  (checked, errors): (&Result<McpServerDraft, Box<McpServerErrors>>, &McpServerErrors),
  unchanged: bool,
  saving: &FutureState<Saved, Message>,
) -> Option<Arc<str>> {
  if let Some(error) = saving.error.as_ref().filter(|_| saving.is_rejected()) {
    return Some(ctx.t_args(
      "connections.dialog.reasons.failed",
      [("reason", &*message_text(ctx, error))],
    ));
  }
  if checked.is_ok() {
    return unchanged.then(|| ctx.t("connections.mcp.dialog.unchanged"));
  }
  let first = [
    &errors.name,
    &errors.program,
    &errors.arguments,
    &errors.working_directory,
    &errors.variables,
    &errors.url,
    &errors.bearer,
    &errors.exempt_tools,
  ]
  .into_iter()
  .flatten()
  .next()?;
  Some(ctx.t_args(
    "connections.dialog.reasons.name",
    [("reason", &*lowercase_first(&message_text(ctx, first)))],
  ))
}

/// `text` with its first word small, as it reads after "Save unavailable:"; a first word that is a name or an
/// identifier (capitals or other characters after its first letter) stays as written.
fn lowercase_first(text: &str) -> String {
  let word = text.split_whitespace().next().unwrap_or_default();
  let mut letters = word.chars();
  let plain = letters.next().is_some_and(char::is_uppercase) && letters.all(char::is_lowercase);
  if !plain {
    return text.to_owned();
  }
  let mut chars = text.chars();
  chars
    .next()
    .map_or_else(String::new, |first| first.to_lowercase().chain(chars).collect())
}

fn head(ctx: &mut Ctx, props: &EditorProps, unchanged: bool, saving: &FutureState<Saved, Message>) -> Head {
  let navigation = Navigation::of(ctx);
  let project = props.project.open.project.id();
  let to = move |view: ConnectionsView| navigation.to(Route::Project(project, ProjectPage::Connections(view)));
  let mut crumbs = vec![
    Crumb::to(ctx.t("connections.title"), to(ConnectionsView::default())),
    Crumb::to(
      ctx.t("connections.tabs.mcpServers"),
      to(ConnectionsView::new(ConnectionsTab::McpServers)),
    ),
  ];
  let (title, status) = match props.stored.as_ref() {
    Some(server) => {
      let view = ConnectionsView::new(ConnectionsTab::McpServers).with_selected(server.id());
      crumbs.push(Crumb::to(server.name().as_str(), to(view)));
      crumbs.push(Crumb::here(ctx.t("connections.mcp.dialog.editCrumb")));
      let status = if unchanged {
        (
          Icon::Check,
          PaletteColor::TextMuted,
          ctx.t("connections.mcp.dialog.noChanges"),
        )
      } else {
        (
          Icon::CircleDot,
          PaletteColor::TextSecondary,
          ctx.t("connections.mcp.dialog.unsaved"),
        )
      };
      (ctx.t("connections.mcp.dialog.editTitle"), status)
    }
    None => {
      crumbs.push(Crumb::here(ctx.t("connections.mcp.dialog.addTitle")));
      let status = (
        Icon::CircleDashed,
        PaletteColor::TextMuted,
        ctx.t("connections.dialog.notSaved"),
      );
      (ctx.t("connections.mcp.dialog.addTitle"), status)
    }
  };
  let status = if saving.is_pending() {
    (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      ctx.t("connections.dialog.saving"),
    )
  } else {
    status
  };
  Head {
    crumbs,
    title,
    description: ctx.t_args(
      "connections.mcp.dialog.description",
      [("project", props.project.open.project.name().as_str())],
    ),
    status,
  }
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Services::of(ctx).activity;
  let open = ctx.props::<EditorProps>().project.open.clone();
  ctx.future_action(move |save: Save| {
    let (activity, open) = (activity.clone(), open.clone());
    async move {
      let spec = operations::connection_save(&open, save.draft.name.as_str());
      let store = async move {
        background::run(move || {
          store_in(&save.repo_root, save.id, save.draft, &save.allowed)
            .map(Saved)
            .map_err(|error| error.message())
        })
        .await?
      };
      activity.track(spec, store).await
    }
  })
}
