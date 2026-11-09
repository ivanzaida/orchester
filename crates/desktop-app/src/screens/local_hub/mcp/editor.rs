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
  core::Signal,
  node::Element,
};
use orchester_core::{
  connection::mcp::McpServer,
  id::{ConnectionId, SecretId},
  secret::SecretName,
  sharing::Missing,
};
use orchester_workspace::{
  activity::operations,
  background,
  connections::{
    data::ProjectConnections,
    mcp::{
      Allowed, McpServerDraft, McpServerErrors, McpServerText, RefusedField, set_enabled_in, store_with_values,
      widenings,
    },
  },
  local_archive::Archivable,
  messages::Message,
  readiness::ReadinessStanding,
  tasks::outside::Outside,
};

use super::{
  Props, notes,
  rows::Look,
  secrets::{Asking, Mark, Taken},
};
use crate::{
  screens::{
    connections::{
      mcp_form::fields::Fields,
      widen::{WidenConfirm, WidenLook, WidenProps},
    },
    local_hub::{
      archive,
      panel::{chips_of, frame},
      words::{Unsaved, kind_where_key},
    },
    team_hub::{
      bar::{bar, bar_saying},
      panel::{body, head_named},
      rows as team_rows,
    },
  },
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
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
struct Save {
  root: PathBuf,
  draft: McpServerDraft,
  allowed: Allowed,
  /// The values typed for the secrets its variables take.
  values: super::secrets::Pending,
}

pub(super) struct Editor {
  save: FutureAction<Save, (), Message>,
  /// The field the last refused save is about.
  refused_field: Signal<RefusedField>,
  pub(super) switch: FutureAction<bool, (), Message>,
  /// Asks for the program's file; the field takes it.
  pub(super) choose: FutureAction<(), Option<PathBuf>, Message>,
  pub(super) fields: Fields,
  /// A save was asked for: every field's error shows, also of fields left empty.
  tried: Signal<bool>,
  /// A save that widens what agents may do waits for the person's answer.
  confirming: Signal<bool>,
  pub(super) asking: Asking,
  /// The arguments in one line, as the panel draws them: split where there is a space.
  pub(super) arguments_line: Signal<String>,
  /// Whether an argument has a space in it, which one line cannot say: the arguments are one a line then.
  pub(super) one_line_arguments: bool,
  /// The tools that run without asking, in one line.
  pub(super) tools_line: Signal<String>,
  /// What the list says of the edits, which this panel clears when it goes.
  unsaved: UnsavedSignal,
  server: ConnectionId,
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
    let refused_field = ctx.signal(RefusedField::None);
    let save = {
      let (services, revision, refused_field) = (services.clone(), props.revision.clone(), refused_field.clone());
      let (id, open) = (props.server.id(), props.props.project.open.clone());
      ctx.future_action(move |save: Save| {
        let (services, revision, open, refused_field) =
          (services.clone(), revision.clone(), open.clone(), refused_field.clone());
        async move {
          let spec = operations::connection_save(&open, save.draft.name.as_str());
          let vault = services.vault.clone();
          let refused_field = refused_field.clone();
          let stored = async move {
            background::run(move || {
              store_with_values(
                &save.root,
                vault.get(),
                (id, save.draft, &save.allowed),
                &save.values.values(),
              )
              .map_err(|error| {
                refused_field.set(error.field());
                error.message()
              })
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
      let (id, services, root) = (props.server.id(), services.clone(), root);
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
    let picker = PathPicker::of(ctx);
    let title = ctx.t("team_hub.fix.programTitle");
    let choose = ctx.future_action(move |(): ()| {
      let (picker, title) = (picker.clone(), title.clone());
      async move { Ok(picker.pick(PathKind::File, title).await) }
    });
    let fields = Fields::new(ctx, &props.text);
    let program = fields.program.clone();
    ctx.watch(&choose.state(), move |state| {
      if let Some(Some(path)) = &state.data {
        program.set(path.to_string_lossy().into_owned());
      }
    });
    let arguments_line = ctx.signal(props.text.arguments.lines().collect::<Vec<_>>().join(" "));
    let tools_line = ctx.signal(props.text.exempt_tools.lines().collect::<Vec<_>>().join(", "));
    let one_line_arguments = !props
      .text
      .arguments
      .lines()
      .any(|line| line.contains(char::is_whitespace));
    let (arguments, line) = (fields.arguments.clone(), arguments_line.clone());
    if one_line_arguments {
      ctx.watch(&line, move |line| {
        arguments.set(line.split_whitespace().collect::<Vec<_>>().join("\n"))
      });
    }
    let (tools, line) = (fields.exempt_tools.clone(), tools_line.clone());
    ctx.watch(&line, move |line| {
      tools.set(
        line
          .split(',')
          .map(str::trim)
          .filter(|tool| !tool.is_empty())
          .collect::<Vec<_>>()
          .join("\n"),
      );
    });
    Self {
      save,
      refused_field,
      switch,
      choose,
      fields,
      tried: ctx.signal(false),
      confirming: ctx.signal(false),
      asking: Asking::new(ctx),
      arguments_line,
      one_line_arguments,
      tools_line,
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
    let typed = self.fields.text();
    let pending = &self.asking.pending;
    let changed = typed != text || !pending.is_empty();
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
    let entry = &props.entry;
    let secrets = taken_secrets(entry, &server, checked.as_ref().ok(), &listed);
    let field = refused.map(|_| self.refused_field.get());
    let marked = match &field {
      Some(RefusedField::Secret(_)) => Mark::Refused,
      _ => Mark::None,
    };
    let look = Look {
      saved: &text,
      typed: &typed,
      listed: &listed,
      errors: &errors,
      tried: self.tried.get(),
      program: match &entry.standing {
        Some(ReadinessStanding::Missing(readiness)) => Some(readiness.missing()),
        Some(ReadinessStanding::Ready) => Some(&[][..]),
        _ => None,
      },
      secrets: (&secrets, marked),
      program_refused: field == Some(RefusedField::Program),
    };
    let mut rows = self.server_rows(ctx, &look);
    rows.extend(self.after_rows(ctx, &look, server.enabled()));
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
    let used =
      (!entry.used_by.is_empty()).then(|| ctx.t_args("local_hub.panel.usedBy", [("names", entry.used_by.join(", "))]));
    let note = match (refused, &field) {
      (Some(message), Some(field)) => Some(notes::refusal(ctx, &entry.name, (message, field))),
      (..) => None,
    }
    .or_else(|| {
      changed.then(|| notes::editing(ctx, &entry.name, &self.changed_labels(ctx, (&typed, &text), &secrets)))
    });
    let body = body()
      .child(head_named(
        &entry.name,
        (
          ctx.t(kind_where_key(entry.kind)).to_string(),
          Some(
            ctx
              .t_args(
                "local_hub.mcp.added",
                [(
                  "date",
                  crate::screens::team_hub::words::date(
                    ctx,
                    orchester_core::time::Timestamp::now(),
                    server.created_at(),
                  )
                  .to_string(),
                )],
              )
              .to_string(),
          ),
        ),
        Some(more),
      ))
      .with_children(note)
      .child(chips_of(notes::chips(ctx, entry, changed), used))
      .child(team_rows::section(
        "local-hub-mcp-server",
        &ctx.t("connections.mcp.dialog.server"),
        Vec::new(),
        rows,
      ))
      .with_children(notes::not_ready(ctx, entry))
      .with_children(self.asking.dialog(ctx));
    let bar = self.bar(
      ctx,
      (&props, &server),
      (&checked, changed, &secrets),
      (field.as_ref(), &revision),
    );
    frame(what, body, bar)
  }
}

impl Drop for Editor {
  fn drop(&mut self) {
    self.asking.wipe();
    // The panel is gone, with the edits: the list must not go on saying they are there.
    if self
      .unsaved
      .get_untracked()
      .is_some_and(|(what, _)| what == Archivable::McpServer(self.server))
    {
      self.unsaved.set(None);
    }
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
    for secret in secrets.iter().filter(|secret| self.asking.pending.has(secret.id)) {
      labels.push(ctx.t_args("local_hub.mcp.secretChanged", [("name", secret.name.to_string())]));
    }
    labels
  }

  /// *Save* and *Cancel*, or the question a save that widens asks first; at rest the fix of what is missing.
  fn bar(
    &self,
    ctx: &mut Ctx,
    (props, server): (&Props, &McpServer),
    (checked, changed, secrets): (&Result<McpServerDraft, Box<McpServerErrors>>, bool, &[Taken]),
    (field, revision): (Option<&RefusedField>, &Signal<u64>),
  ) -> Option<Element> {
    let start = checked.as_ref().ok().filter(|_| changed).map(|draft| Save {
      root: props.project.open.repo_root.clone(),
      draft: draft.clone(),
      allowed: Allowed::These(Vec::new()),
      values: self.asking.pending.clone(),
    });
    if self.confirming.get()
      && let Some(start) = &start
    {
      return Some(self.confirm(ctx, server, start));
    }
    if !changed {
      return self.fix(ctx, &props.entry, secrets);
    }
    let reset = revision.clone();
    let cancel = ActionButton::new(
      "local-hub-mcp-cancel",
      ButtonKind::Secondary,
      ctx.t("local_hub.mcp.cancel"),
    )
    .icon(Icon::X)
    .on_click(move || reset.update(|count| *count += 1));
    let button = ActionButton::new("local-hub-mcp-save", ButtonKind::Primary, ctx.t("local_hub.mcp.save"))
      .icon(Icon::Save)
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
      (false, None) => button.on_click(move || tried.set(true)),
    };
    let hint = match field {
      Some(RefusedField::Secret(_)) => (ctx.t("local_hub.mcp.allowThenSave").to_string(), PaletteColor::Danger),
      _ => (ctx.t("local_hub.mcp.onlyHere").to_string(), PaletteColor::TextMuted),
    };
    Some(bar_saying(vec![button.into(), cancel.into()], Some(hint)))
  }

  /// At rest, the fix of the first thing missing: the program's path, or a secret's value.
  fn fix(&self, ctx: &mut Ctx, entry: &super::super::model::Entry, secrets: &[Taken]) -> Option<Element> {
    let Some(ReadinessStanding::Missing(readiness)) = &entry.standing else {
      return None;
    };
    let button = match readiness.missing().first()? {
      Missing::LocalMcpProgram { .. } => {
        let choose = self.choose.clone();
        ActionButton::new(
          "local-hub-mcp-set-program",
          ButtonKind::Primary,
          ctx.t("local_hub.mcp.setProgram"),
        )
        .icon(Icon::FolderOpen)
        .pinned()
        .on_click(move || choose.run(()))
      }
      Missing::LocalSecret { secret, name } => {
        let taken = secrets.iter().find(|taken| taken.id == *secret)?.clone();
        let asking = self.asking.clone();
        ActionButton::new(
          "local-hub-mcp-set-secret",
          ButtonKind::Primary,
          ctx.t_args("local_hub.mcp.setSecret", [("name", name.to_string())]),
        )
        .icon(Icon::KeyRound)
        .pinned()
        .on_click(move || asking.ask(&taken))
      }
      _ => return None,
    };
    Some(bar(
      vec![button.into()],
      Some(ctx.t("local_hub.mcp.askedOnce").to_string()),
    ))
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
    Some(draft) => draft.environment.secrets().collect(),
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
