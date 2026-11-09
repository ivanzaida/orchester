//! The editor of a local MCP server's panel: the fields as typed over what was saved, *Save* and *Cancel*, the switch,
//! and what a save that widens asks first. The fields are [`Fields`]; the rows, the notes and the secret values are
//! [`super::rows`], [`super::notes`] and [`super::secrets`].

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_core::{
  connection::mcp::McpServer,
  id::{ConnectionId, SecretId},
  secret::SecretName,
  sharing::Missing,
  time::Timestamp,
};
use orchester_workspace::{
  connections::{
    data::ProjectConnections,
    mcp::{Allowed, McpServerDraft, McpServerErrors, McpServerText, RefusedField, widenings},
  },
  local_archive::Archivable,
  messages::Message,
  readiness::ReadinessStanding,
};

use super::{
  Props, actions,
  form::Form,
  notes,
  rows::Look,
  secrets::{Mark, Taken},
};
use crate::{
  screens::{
    connections::widen::{WidenConfirm, WidenLook, WidenProps},
    local_hub::{
      archive,
      leave::{clear_unsaved, mark_unsaved},
      panel::{chips_of, frame},
      save_bar::{self, Save as SaveNow},
      words::{Unsaved, kind_where_key},
    },
    team_hub::{
      panel::{body, head_named},
      rows as team_rows,
      words::date,
    },
  },
  services::Services,
  ui::{message::message_text, pressable::Press},
};

/// Which MCP server has edits that are not saved, and how: read by the list, written by the panel.
pub(in crate::screens::local_hub) type UnsavedSignal = Signal<Option<(Archivable, Unsaved)>>;

#[derive(Clone)]
pub(super) struct EditorProps {
  pub props: Props,
  pub server: McpServer,
  /// The fields as saved.
  pub text: McpServerText,
  pub listed: ProjectConnections,
  pub revision: Signal<u64>,
}
impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.props == other.props && self.server == other.server && self.text == other.text && self.listed == other.listed
  }
}
impl DevtoolsInspectable for EditorProps {}

/// What a save stores, and what it may widen.
#[derive(Clone)]
pub(super) struct Save {
  pub(super) root: PathBuf,
  pub(super) draft: McpServerDraft,
  pub(super) allowed: Allowed,
  /// The values typed for the secrets its variables take.
  pub(super) values: super::secrets::Pending,
}

/// What the last refused save is about, as the panel marks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
  None,
  Program,
  /// The value of a secret it took.
  Secret,
}
impl DevtoolsInspectable for Refusal {}

impl Refusal {
  pub(super) fn of(field: &RefusedField) -> Self {
    match field {
      RefusedField::Program => Self::Program,
      RefusedField::Secret(_) => Self::Secret,
      RefusedField::None => Self::None,
    }
  }
}

pub(super) struct Editor {
  save: FutureAction<Save, (), Message>,
  /// The field the last refused save is about.
  refused_field: Signal<Refusal>,
  pub(super) switch: FutureAction<bool, (), Message>,
  pub(super) form: Form,
  /// A save that widens what agents may do waits for the person's answer.
  confirming: Signal<bool>,
  /// What the list says of the edits, which this panel clears when it goes.
  unsaved: UnsavedSignal,
  server: ConnectionId,
}

impl Component for Editor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let services = Services::of(ctx);
    let refused_field = ctx.signal(Refusal::None);
    Self {
      save: actions::save(ctx, &props, &services, &refused_field),
      refused_field,
      switch: actions::switch(ctx, &props, &services),
      form: Form::new(ctx, &props.text),
      confirming: ctx.signal(false),
      unsaved: props.props.unsaved.clone(),
      server: props.server.id(),
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
    let typed = self.form.fields.text();
    let pending = &self.form.asking.pending;
    let changed = typed != text || !pending.is_empty();
    let taken: Vec<_> = listed
      .mcp_names
      .iter()
      .filter(|(id, _)| *id != server.id())
      .map(|(_, name)| name.clone())
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
    mark_unsaved(&props.unsaved, what, (changed, refused.is_some()));
    let entry = &props.entry;
    let secrets = taken_secrets(entry, &server, checked.as_ref().ok(), &listed);
    let field = refused.map(|_| self.refused_field.get());
    let marked = match &field {
      Some(Refusal::Secret) => Mark::Refused,
      _ => Mark::None,
    };
    let look = Look {
      saved: &text,
      typed: &typed,
      listed: &listed,
      errors: &errors,
      // A field that is wrong says so while there is something to save: *Save* waits for it.
      tried: changed,
      program: match &entry.standing {
        Some(ReadinessStanding::Missing(readiness)) => Some(readiness.missing()),
        Some(ReadinessStanding::Ready) => Some(&[][..]),
        _ => None,
      },
      secrets: (&secrets, marked),
      program_refused: field == Some(Refusal::Program),
      creating: false,
    };
    let mut rows = self.form.server_rows(ctx, &look);
    rows.extend(self.after_rows(ctx, &look, server.enabled()));
    let used = notes::used_by(ctx, entry);
    let note = match (refused, &field) {
      (Some(message), Some(field)) => Some(notes::refusal(ctx, &entry.name, (message, *field == Refusal::Secret))),
      (..) => None,
    }
    .or_else(|| {
      changed.then(|| notes::editing(ctx, &entry.name, &self.changed_labels(ctx, (&typed, &text), &secrets)))
    });
    let body = body()
      .child(head(ctx, &props, Some(server.created_at())))
      .with_children(note)
      .child(chips_of(notes::chips(ctx, entry, changed), used))
      .child(team_rows::section(
        "local-hub-mcp-server",
        &ctx.t("connections.mcp.dialog.server"),
        Vec::new(),
        rows,
      ))
      .with_children(notes::not_ready(ctx, entry))
      .with_children(self.form.asking.dialog(ctx, false));
    let bar = self.bar(ctx, (&props, &server), (&checked, changed), (field, &revision));
    frame(what, body, Some(bar))
  }
}

/// The head: *MCP server · added Oct 30* over the name, with the … menu; when it was `added` is `None` while the server
/// is read for the first time.
pub(super) fn head(ctx: &mut Ctx, props: &Props, added: Option<Timestamp>) -> Row {
  let entry = &props.entry;
  let more = archive::menu(
    ctx,
    archive::Target {
      root: props.project.open.repo_root.clone(),
      project: props.project.open.project.id(),
      what: Archivable::McpServer(props.server),
      kind: entry.kind,
      name: Arc::from(entry.name.as_str()),
    },
  );
  let added = added.map(|at| {
    let date = date(ctx, Timestamp::now(), at);
    ctx
      .t_args("local_hub.mcp.added", [("date", date.to_string())])
      .to_string()
  });
  head_named(
    &entry.name,
    (ctx.t(kind_where_key(entry.kind)).to_string(), added),
    Some(more),
  )
}

impl Drop for Editor {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, Archivable::McpServer(self.server));
  }
}

impl Editor {
  /// What differs from what was saved, by the name of its row.
  fn changed_labels(
    &self,
    ctx: &Ctx,
    (typed, saved): (&McpServerText, &McpServerText),
    secrets: &[Taken],
  ) -> Vec<Arc<str>> {
    let mut labels = Vec::new();
    let mut note = |differs: bool, key: &str| {
      if differs {
        labels.push(ctx.t(key));
      }
    };
    note(typed.transport != saved.transport, "local_hub.mcp.runsAs");
    note(typed.program != saved.program, "connections.mcp.property.program");
    note(typed.arguments != saved.arguments, "connections.mcp.property.arguments");
    note(
      typed.working_directory != saved.working_directory,
      "connections.mcp.property.folder",
    );
    note(typed.exempt_tools != saved.exempt_tools, "local_hub.mcp.tools");
    note(typed.variables != saved.variables, "connections.mcp.variables");
    note(
      typed.url != saved.url || typed.authorization != saved.authorization,
      "connections.mcp.property.endpoint",
    );
    for secret in secrets.iter().filter(|secret| self.form.asking.pending.has(secret.id)) {
      labels.push(ctx.t_args("local_hub.mcp.secretChanged", [("name", secret.name.to_string())]));
    }
    labels
  }

  /// *Save* and *Cancel*, always there and off while nothing differs, or the question a save that widens asks first.
  /// *Save* waits, saying why, while the fields cannot be saved; every field then says what is wrong with it.
  fn bar(
    &self,
    ctx: &mut Ctx,
    (props, server): (&Props, &McpServer),
    (checked, changed): (&Result<McpServerDraft, Box<McpServerErrors>>, bool),
    (field, revision): (Option<Refusal>, &Signal<u64>),
  ) -> Element {
    let start = checked.as_ref().ok().filter(|_| changed).map(|draft| Save {
      root: props.project.open.repo_root.clone(),
      draft: draft.clone(),
      allowed: Allowed::These(Vec::new()),
      values: self.form.asking.pending.clone(),
    });
    if self.confirming.get()
      && let Some(start) = &start
    {
      return self.confirm(ctx, server, start);
    }
    let pending = self.save.state().get().is_pending();
    let save = match (pending, changed, start) {
      (true, ..) => SaveNow::Saving,
      (false, false, _) => SaveNow::Unchanged,
      (false, true, None) => SaveNow::Waits(ctx.t("local_hub.edit.invalid")),
      (false, true, Some(start)) => {
        let widens = !widenings(Some(server), &start.draft).is_empty();
        let (save, confirming) = (self.save.clone(), self.confirming.clone());
        SaveNow::Ready(Press::run(move || {
          if widens {
            confirming.set(true);
          } else {
            save.run(start.clone());
          }
        }))
      }
    };
    let reset = revision.clone();
    let cancel = (changed && !pending).then(|| Press::run(move || reset.update(|count| *count += 1)));
    let hint = match field {
      Some(Refusal::Secret) => (ctx.t("local_hub.mcp.allowThenSave").to_string(), PaletteColor::Danger),
      _ => (ctx.t("local_hub.mcp.onlyHere").to_string(), PaletteColor::TextMuted),
    };
    save_bar::bar(ctx, "local-hub-mcp", (save, cancel), hint)
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

/// The secrets the variables take: those of the draft while the fields make one, else the server's own; whether the
/// secret store holds a value is the server's standing.
fn taken_secrets(
  entry: &super::super::model::Entry,
  server: &McpServer,
  draft: Option<&McpServerDraft>,
  listed: &ProjectConnections,
) -> Vec<Taken> {
  let ids: Vec<SecretId> = match draft {
    Some(draft) => draft.secrets(),
    None => server.secrets(),
  };
  let lacking: Vec<&SecretName> = match &entry.standing {
    Some(ReadinessStanding::Missing(readiness)) => readiness
      .missing()
      .iter()
      .filter_map(|part| match part {
        Missing::LocalSecret { name, .. } => Some(name),
        _ => None,
      })
      .collect(),
    _ => Vec::new(),
  };
  ids
    .into_iter()
    .filter_map(|id| listed.secrets.iter().find(|secret| secret.id() == id))
    .map(|secret| Taken {
      id: secret.id(),
      name: secret.name().clone(),
      has_value: !lacking.contains(&secret.name()),
    })
    .collect()
}
