//! *Let the Orchestrator drive* (the row of `S03` that replaced the six switches `Publish drafts` … `Write with git`,
//! ORC-909): the project's own choice of whether its Orchestrator chat acts itself (ORC-594), read through its query,
//! which the project's loader read as the project opened, and saved off the UI thread; until it is first read and
//! while it saves, the switch cannot be flipped. Only a person flips it: the row refuses lurq's synthetic input and
//! says so under itself, and while it is mounted the app's MCP server refuses window input (`mcp::input_gate`). Off
//! holds in the project from the person's click, before the save runs and if it fails.

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    events::{InputSource, input_source},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::Workspace,
};

use super::super::{Status, TabProps, kit::SettingRow};
use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
  queries::{self, Session, project},
  services::Services,
  ui::{
    field::field_error,
    icon::Icon,
    message::message_text,
    setting_notice::status_line,
    skeleton,
    toggle::{Toggle, ToggleKind},
  },
};

/// The switch's element id, which its row's parts extend.
pub(super) const ID: &str = "settings-orchestrator-drive";

/// Its name in the developer log.
const LOGGED: &str = "Let the Orchestrator drive";

type SaveAction = FutureAction<bool, bool, Message>;

pub(super) struct Switch {
  /// The setting as the person last chose it; `None` until they do.
  saved: Signal<Option<bool>>,
  save: SaveAction,
  /// While the row is mounted, the MCP server refuses window input.
  _protected: Protected,
}

impl Component for Switch {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = ctx.props::<TabProps>().clone();
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    let save = save_action(ctx, tab.project.open.repo_root.clone());
    let status = tab.status;
    ctx.watch(&save.state(), move |state| {
      status.set(match (state.is_pending(), &state.error) {
        (true, _) => Status::Saving,
        (false, Some(_)) => Status::Failed,
        (false, None) => Status::Saved,
      });
    });
    Self {
      saved: ctx.signal(None),
      save,
      _protected: gate.protect(Guarded::PermissionsSetting),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let tab = ctx.props::<TabProps>().clone();
    let root = tab.project.open.repo_root;
    let session = Session::of(ctx);
    let workspace = session.0.workspace.clone();
    let state = self.save.state().get();
    let read = queries::state(&ctx.query(project::orchestrator_drives(session, root.clone())));
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice until the stored value is it again; the stored value until they choose, or once a save
    // failed.
    queries::settle(&self.saved, read.data.as_ref(), state.is_pending());
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    // The row takes no change before it lived through one pass after mounting: an input meant for what the window
    // showed before cannot land on it.
    let armed = ctx
      .future((), |()| async { Ok::<(), Message>(()) })
      .state()
      .get()
      .data
      .is_some();
    let switch = Toggle::new(
      ID,
      ToggleKind::Switch,
      shown.unwrap_or(false),
      ctx.t("project_settings.orchestrator.drive.label"),
    )
    .hide_label();
    let switch = if shown.is_some() && !fixed && armed {
      self.listen(switch, (workspace, root))
    } else {
      switch
    };
    // While the stored choice is read, the switch is a bone of its size.
    let control: Element = if shown.is_none() && read.is_pending() {
      skeleton::switch().into()
    } else {
      switch.into()
    };
    let problem = match (&read.error, &state.error) {
      (Some(reason), _) => Some(("project_settings.orchestrator.unreadable", reason)),
      (None, Some(reason)) => Some(("project_settings.orchestrator.notSaved", reason)),
      (None, None) => None,
    }
    .map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      Element::from(field_error(
        &format!("{ID}-error"),
        &ctx.t_args(key, [("reason", &*reason)]),
      ))
    });
    // What the Orchestrator does once it drives, then why the row cannot be read or was not saved.
    let said = (shown == Some(true)).then(|| {
      Element::from(status_line(
        &format!("{ID}-on"),
        Icon::MessagesSquare,
        &ctx.t("project_settings.orchestrator.drive.on"),
      ))
    });
    SettingRow::new(ID, ctx.t("project_settings.orchestrator.drive.label"))
      .hint(ctx.t("project_settings.orchestrator.drive.hint"))
      .info(ctx.t("project_settings.orchestrator.drive.info"))
      .control(control)
      .under_all(said)
      .under_all(problem)
      .render(ctx)
  }
}

impl Switch {
  /// `switch`, saving what a person chooses for the project at `root`. Synthetic input never changes it: the gate
  /// refuses it while the row is mounted, and what reaches the switch all the same is only logged.
  fn listen(&self, switch: Toggle, (workspace, root): (Workspace, PathBuf)) -> Toggle {
    let (saved, save) = (self.saved.clone(), self.save.clone());
    switch.on_change(move |chosen| {
      if input_source() == InputSource::Synthetic {
        tracing::warn!(
          setting = LOGGED,
          "synthetic input reached a person's setting; not taken"
        );
        return;
      }
      if !chosen {
        // Off holds from the person's click, before the save runs and if it fails.
        workspace.revoke_orchestrator_drive(&root);
      }
      saved.set(Some(chosen));
      save.run(chosen);
    })
  }
}

/// Saves the choice the person made for the project at `root`; the workspace counts the change, which reads the
/// project's switch again (`queries`).
fn save_action(ctx: &mut Ctx, root: PathBuf) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |drives: bool| {
    let (workspace, root) = (workspace.clone(), root.clone());
    async move {
      background::run(move || workspace.set_orchestrator_drives(&root, drives))
        .await?
        .map(|()| drives)
        .map_err(|error| {
          tracing::error!(%error, setting = LOGGED, "a person's setting was not saved");
          messages::store_error(&error)
        })
    }
  })
}
