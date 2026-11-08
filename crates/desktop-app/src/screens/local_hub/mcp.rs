//! A local MCP server's panel (`Agents · MCP servers` L2 `IpzZ7`, editing L8 `u6EkvN`, save refused L12 `uzwOD`): the
//! server's fields in one panel, every one the person's own and editable in place. A local MCP server has no versions:
//! no draft, no Publish, no Versions. Nothing changes until *Save*, which applies it to the project at once; *Cancel*
//! puts back what was saved. Between the two the row and the panel say *Unsaved changes*; a save that fails says why in
//! place, marks nothing as saved and keeps everything typed. Archive is in the head's … menu.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{connection::mcp::McpServer, id::ConnectionId};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::ProjectConnections,
    mcp::{Allowed, McpServerDraft, McpServerText, set_enabled_in, store_in, widenings},
  },
  local_archive::Archivable,
  messages::Message,
  tasks::outside::Outside,
};

use super::{
  archive,
  model::Entry,
  panel::{chips_of, frame, readiness},
  words::{Unsaved, kind_where_key},
};
use crate::{
  project::ShownProject,
  queries,
  screens::{
    connections::{
      data,
      mcp_form::fields::Fields,
      widen::{WidenConfirm, WidenLook, WidenProps},
    },
    team_hub::{
      bar::bar,
      panel::{body, head_named},
    },
  },
  services::Services,
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::BadgeTone,
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    state::loading,
    toggle::{Toggle, ToggleKind},
  },
};

/// Which MCP server has edits that are not saved, and how: read by the list, written by the panel.
pub(super) type UnsavedSignal = Signal<Option<(Archivable, Unsaved)>>;

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  pub server: ConnectionId,
  pub unsaved: UnsavedSignal,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.entry == other.entry && self.server == other.server
  }
}
impl DevtoolsInspectable for Props {}

/// The panel of the local MCP server `props.server`.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("local-hub-mcp-{}", props.server);
  ctx.mount_keyed::<Loader>(&key, props)
}

/// Reads the project's connections (the server and the secrets its variables can take) and shows the editor over them.
struct Loader {
  /// Counts the saves and the cancels, each of which opens the editor again on what is saved.
  revision: Signal<u64>,
}

impl Component for Loader {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      revision: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let query = data::load(ctx, props.project.open.repo_root.clone());
    let loaded = queries::state(&query);
    let Some(listed) = loaded.data else {
      return loading("local-hub-mcp-loading", &ctx.t("local_hub.reading")).into();
    };
    let Some(server) = listed.mcp.iter().find(|server| server.id() == props.server).cloned() else {
      return Text::new(&ctx.t("local_hub.mcp.gone"))
        .id("local-hub-mcp-gone")
        .variant(HubText::Value.role())
        .into();
    };
    let text = McpServerText::of(&server, &listed.secrets);
    let key = format!("local-hub-mcp-edit-{}-{}", props.server, self.revision.get());
    ctx.mount_keyed::<Editor>(
      &key,
      EditorProps {
        props,
        server,
        text,
        listed,
        revision: self.revision.clone(),
      },
    )
  }
}

#[derive(Clone)]
struct EditorProps {
  props: Props,
  server: McpServer,
  /// The fields as saved.
  text: McpServerText,
  listed: ProjectConnections,
  revision: Signal<u64>,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.props == other.props && self.server == other.server && self.text == other.text && self.listed == other.listed
  }
}
impl DevtoolsInspectable for EditorProps {}

/// What a save stores, and what it may widen.
#[derive(Clone)]
struct Save {
  root: PathBuf,
  draft: McpServerDraft,
  allowed: Allowed,
}

struct Editor {
  save: FutureAction<Save, (), Message>,
  switch: FutureAction<bool, (), Message>,
  fields: Fields,
  /// A save was asked for: every field's error shows, also of fields left empty.
  tried: Signal<bool>,
  /// A save that widens what agents may do waits for the person's answer.
  confirming: Signal<bool>,
}

impl Component for Editor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let services = Services::of(ctx);
    let (project, root) = (
      props.props.project.open.project.id(),
      props.props.project.open.repo_root.clone(),
    );
    let save = {
      let (services, project, revision) = (services.clone(), project, props.revision.clone());
      let (id, open) = (props.server.id(), props.props.project.open.clone());
      ctx.future_action(move |save: Save| {
        let (services, revision, open) = (services.clone(), revision.clone(), open.clone());
        async move {
          let spec = operations::connection_save(&open, save.draft.name.as_str());
          let stored = async move {
            background::run(move || {
              store_in(&save.root, Some(id), save.draft, &save.allowed)
                .map(drop)
                .map_err(|error| error.message())
            })
            .await?
          };
          services.activity.track(spec, stored).await?;
          services.changes.record(project, Outside::Connections);
          // The editor opens again on what was saved, with nothing left unsaved.
          revision.update(|count| *count += 1);
          Ok(())
        }
      })
    };
    let switch = {
      let (id, services) = (props.server.id(), services);
      ctx.future_action(move |enabled: bool| {
        let (root, services) = (root.clone(), services.clone());
        async move {
          background::run(move || {
            set_enabled_in(&root, id, enabled).map_err(|error| orchester_workspace::messages::store_error(&error))
          })
          .await??;
          services.changes.record(project, Outside::Connections);
          Ok(())
        }
      })
    };
    Self {
      save,
      switch,
      fields: Fields::new(ctx, &props.text),
      tried: ctx.signal(false),
      confirming: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let EditorProps {
      props,
      server,
      text,
      listed,
      revision,
    } = ctx.props::<EditorProps>().clone();
    let typed = self.fields.text();
    let changed = typed != text;
    let taken: Vec<_> = listed
      .mcp
      .iter()
      .filter(|other| other.id() != server.id())
      .map(|other| other.name().clone())
      .collect();
    let checked = typed.draft(&taken, &listed.secrets);
    let errors = checked
      .as_ref()
      .err()
      .map(|errors| (**errors).clone())
      .unwrap_or_default();
    let saving = self.save.state().get();
    let refused = saving.error.as_ref().filter(|_| saving.is_rejected());
    let what = Archivable::McpServer(server.id());
    let mark = match (changed, refused.is_some()) {
      (true, true) => Some(Unsaved::Refused),
      (true, false) => Some(Unsaved::Edited),
      (false, _) => None,
    };
    // The list says it too; set only when it differs so the page is not read again for nothing.
    let wanted = mark.map(|mark| (what, mark));
    if props.unsaved.get_untracked() != wanted {
      props.unsaved.set(wanted);
    }
    let sections = self.fields.sections(ctx, (&listed, &errors), self.tried.get(), false);
    let entry = &props.entry;
    let more = archive::menu(
      ctx,
      archive::Target {
        root: props.project.open.repo_root.clone(),
        project: props.project.open.project.id(),
        what,
        kind: entry.kind,
        name: Arc::from(entry.name.as_str()),
      },
    );
    let mut chips = Vec::new();
    if changed {
      chips.push((ctx.t("local_hub.chip.unsaved"), BadgeTone::Warning));
    }
    chips.push(match entry.group {
      orchester_workspace::readiness::Group::Ready => (ctx.t("local_hub.chip.ready"), BadgeTone::Success),
      orchester_workspace::readiness::Group::OffHere => (ctx.t("local_hub.chip.off"), BadgeTone::Neutral),
      orchester_workspace::readiness::Group::NeedsYouHere => (
        entry.standing.as_ref().map_or_else(
          || ctx.t("local_hub.chip.ready"),
          |standing| super::words::needs(ctx, entry, standing),
        ),
        BadgeTone::Warning,
      ),
    });
    let used =
      (!entry.used_by.is_empty()).then(|| ctx.t_args("local_hub.panel.usedBy", [("names", entry.used_by.join(", "))]));
    let body = body()
      .child(head_named(
        &entry.name,
        (ctx.t(kind_where_key(entry.kind)).to_string(), None),
        Some(more),
      ))
      .child(chips_of(chips, used))
      .with_children(refused.map(|error| refusal(ctx, &server, error)))
      .with_children(changed.then(|| unsaved_line(ctx)))
      .with_children(readiness(ctx, entry))
      .with_children(sections)
      .child(self.switch_row(ctx, &server));
    let bar = self.bar(ctx, (&props, &server), (&checked, changed), &revision);
    frame(what, body, Some(bar))
  }
}

impl Editor {
  /// *On · Runs may use it*: turns the server on or off for every profile that chooses it, at once.
  fn switch_row(&self, ctx: &Ctx, server: &McpServer) -> Row {
    let switch = self.switch.clone();
    let toggle = Toggle::new(
      "local-hub-mcp-on",
      ToggleKind::Switch,
      server.enabled(),
      ctx.t("local_hub.mcp.on"),
    )
    .hide_label()
    .on_change(move |on| switch.run(on));
    Row::new()
      .id("local-hub-mcp-on-row")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("local_hub.mcp.on"))
          .variant(HubText::PanelLabel.role())
          .width(Dimension::Px(160.0)),
      )
      .child(toggle)
      .child(
        Text::new(&ctx.t("local_hub.mcp.onHint"))
          .variant(HubText::Value.role())
          .flex(1.0),
      )
  }

  /// *Cancel* and *Save*, or the question a save that widens asks first.
  fn bar(
    &self,
    ctx: &mut Ctx,
    (props, server): (&Props, &McpServer),
    (checked, changed): (
      &Result<McpServerDraft, Box<orchester_workspace::connections::mcp::McpServerErrors>>,
      bool,
    ),
    revision: &Signal<u64>,
  ) -> Element {
    let start = checked.as_ref().ok().filter(|_| changed).map(|draft| Save {
      root: props.project.open.repo_root.clone(),
      draft: draft.clone(),
      allowed: Allowed::These(Vec::new()),
    });
    if self.confirming.get()
      && let Some(start) = &start
    {
      return self.confirm(ctx, server, start);
    }
    let reset = revision.clone();
    let cancel = ActionButton::new("local-hub-mcp-cancel", ButtonKind::Ghost, ctx.t("local_hub.mcp.cancel"))
      .on_click(move || reset.update(|count| *count += 1));
    let cancel = if changed { cancel } else { cancel.disabled() };
    let button = ActionButton::new("local-hub-mcp-save", ButtonKind::Primary, ctx.t("local_hub.mcp.save"))
      .icon(Icon::Check)
      .pinned();
    let tried = self.tried.clone();
    let button = match (self.save.state().get().is_pending(), start) {
      (true, _) => button.loading(),
      (false, Some(start)) => {
        let widens = !widenings(Some(server), &start.draft).is_empty();
        let (save, confirming) = (self.save.clone(), self.confirming.clone());
        button.on_click(move || {
          if widens {
            confirming.set(true);
          } else {
            save.run(start.clone());
          }
        })
      }
      (false, None) if changed => button.on_click(move || tried.set(true)),
      (false, None) => button.disabled(),
    };
    let hint = ctx.t(if changed {
      "local_hub.mcp.saveHint"
    } else {
      "local_hub.mcp.onlyHere"
    });
    bar(vec![cancel.into(), button.into()], Some(hint.to_string()))
  }

  /// What saving widens, for the person to confirm in place of the actions.
  fn confirm(&self, ctx: &mut Ctx, server: &McpServer, start: &Save) -> Element {
    let shown = widenings(Some(server), &start.draft);
    let reasons: Arc<[Arc<str>]> = shown.iter().map(|reason| message_text(ctx, reason)).collect();
    let (save, done, keep) = (self.save.clone(), self.confirming.clone(), self.confirming.clone());
    let start = start.clone();
    let question = WidenProps {
      look: WidenLook::Box,
      affirming: false,
      automation: crate::mcp::input_gate::Guarded::PermissionWidening,
      id: "local-hub-mcp-confirm-save".into(),
      title: ctx.t("connections.mcp.dialog.widenTitle"),
      reasons,
      confirm: ctx.t("local_hub.mcp.save"),
      keep: ctx.t("connections.mcp.dialog.keepEditing"),
      on_confirm: Arc::new(move || {
        done.set(false);
        save.run(Save {
          allowed: Allowed::These(shown.clone()),
          ..start.clone()
        });
      }),
      on_keep: Arc::new(move || keep.set(false)),
    };
    ctx.mount_keyed::<WidenConfirm>("local-hub-mcp-confirm-save", question)
  }
}

/// *Editing figma · not saved yet* over what Save and Cancel do (`Text` in L8).
fn unsaved_line(ctx: &Ctx) -> Column {
  Column::new()
    .id("local-hub-mcp-unsaved")
    .width(Dimension::Pct(100.0))
    .child(
      Text::new(&ctx.t("local_hub.mcp.unsavedBody"))
        .variant(HubText::PanelHint.role())
        .width(Dimension::Pct(100.0)),
    )
}

/// Why the save did nothing, in place above the form (`Error` in L12).
fn refusal(ctx: &Ctx, server: &McpServer, error: &Message) -> Element {
  Notice::new(
    "local-hub-mcp-refused",
    NoticeTone::Danger,
    ctx.t_args("local_hub.mcp.refusedTitle", [("name", server.name().as_str())]),
  )
  .body(ctx.t_args("local_hub.mcp.refusedBody", [("reason", &*message_text(ctx, error))]))
  .into()
}
