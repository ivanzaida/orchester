//! Add connection (04.02 `h6CmJ`, dialog `IAN4P`), also used to edit one. The provider's CLI
//! is looked up on the `PATH` (or chosen as a file) and checked as soon as it is known; the
//! connection can be saved only when the check found the expected CLI. Checking runs only
//! the CLI's read-only commands (`orchester_core::connection`). The fields are in `form`; a new
//! connection's type can be switched to GitHub (`github_dialog`), and its kind to a native model API
//! (`native_dialog`).

use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::{
  connection::{AgentProvider, ConnectionCheck},
  id::ConnectionId,
};

use super::{
  DialogContext, DialogMode, KindChoice,
  dialog_frame::{self, SaveBar, dialog, save_reason_of},
  form,
  status::short_key,
};
use crate::{
  activity::{Activity, operations},
  background,
  connections::{
    data::{ConnectionDraft, save_in},
    probe,
  },
  messages::Message,
  ui::form::Submit,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ProviderChoice(pub AgentProvider);

impl lurq::app::component::DevtoolsInspectable for ProviderChoice {}

/// A program file the user chose instead of the one found on the `PATH`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Picked(pub Option<PathBuf>);

impl lurq::app::component::DevtoolsInspectable for Picked {}

/// A check and what it checked, so a result is never shown for other inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Checked {
  provider: AgentProvider,
  executable: PathBuf,
  check: ConnectionCheck,
}

impl lurq::app::component::DevtoolsInspectable for Checked {}

/// The saved connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Saved(ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// The file chosen in the picker, if any.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PickedFile(pub Option<PathBuf>);

impl lurq::app::component::DevtoolsInspectable for PickedFile {}

/// Copies of the CLI found on the `PATH`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Detected(Vec<PathBuf>);

impl lurq::app::component::DevtoolsInspectable for Detected {}

type SaveAction = FutureAction<(PathBuf, ConnectionDraft), Saved, Message>;
pub(super) type PickAction = FutureAction<Arc<str>, PickedFile, Message>;

/// What the dialog knows about the current program.
pub(super) enum Verification {
  Detecting,
  NotFound,
  Checking,
  /// The check could not run to the end (the background work stopped).
  Failed(Message),
  Done(ConnectionCheck),
}

/// The program being set up and what is known about it.
pub(super) struct Program {
  pub provider: AgentProvider,
  pub executable: Option<PathBuf>,
  /// Copies found on the `PATH`.
  pub found: usize,
  pub verification: Verification,
}

pub struct ConnectionDialog {
  pub(super) context: DialogContext,
  pub(super) provider: Signal<ProviderChoice>,
  pub(super) name: Signal<String>,
  pub(super) picked: Signal<Picked>,
  /// Bumped by Recheck to run the check again for the same program.
  pub(super) attempt: Signal<u64>,
  /// The connection type select of a new connection; GitHub switches to the GitHub dialog.
  pub(super) kind: Signal<KindChoice>,
  /// The dialog, focused when it opens so that Tab starts at its first control. The design
  /// shows no field focused (`IAN4P`), so no field takes the focus.
  frame: ElementRef,
}

impl Component for ConnectionDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<DialogContext>()
      .expect("the connections screen provides the dialog context");
    let (provider, name, picked) = match &context.mode {
      DialogMode::EditAgent(connection) => (
        connection.provider(),
        connection.name().as_str().to_owned(),
        connection.executable().map(Path::to_path_buf),
      ),
      mode => {
        let provider = match mode {
          DialogMode::AddAgent(provider) => *provider,
          _ => AgentProvider::CodexCli,
        };
        (provider, suggested_name(ctx, &context, provider), None)
      }
    };
    let dialog = Self {
      provider: ctx.signal(ProviderChoice(provider)),
      name: ctx.signal(name),
      picked: ctx.signal(Picked(picked)),
      attempt: ctx.signal(0),
      kind: ctx.signal(KindChoice::Agent),
      frame: ElementRef::new(),
      context,
    };
    ctx.focus(&dialog.frame);
    dialog.watch(ctx);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Futures are matched by call order: the actions come first, as in `create`.
    let save = save_action(ctx);
    let pick = pick_action(ctx);
    let program = self.program(ctx);
    let draft = self.draft(&program);
    let submit = self.save(&save, draft.as_ref().ok());
    let fields = self.form(ctx, &program, &pick);
    let state = save.state().get();
    let saving = state.is_pending();
    let bar = SaveBar {
      saving,
      reason: save_reason_of(&draft, state.error.filter(|_| !saving)),
      submit,
    };
    let title = match self.context.mode {
      DialogMode::EditAgent(_) => "connections.dialog.editTitle",
      _ => "connections.dialog.addTitle",
    };
    dialog(ctx, &self.frame, &ctx.t(title), &self.context, fields, bar)
  }
}

impl ConnectionDialog {
  /// A new provider gets its own program and, unless the user renamed the connection, its own
  /// suggested name. A saved connection closes the dialog; a chosen file replaces the found one.
  fn watch(&self, ctx: &mut Ctx) {
    let retype = self.context.retype.clone();
    ctx.watch(&self.kind, move |kind| retype(*kind));
    if matches!(self.context.mode, DialogMode::AddAgent(_)) {
      let suggestions = AgentProvider::CLI.map(|provider| suggested_name(ctx, &self.context, provider));
      let (name, picked) = (self.name.clone(), self.picked.clone());
      ctx.watch(&self.provider, move |choice| {
        if suggestions.contains(&name.get_untracked()) {
          let index = AgentProvider::CLI
            .iter()
            .position(|provider| *provider == choice.0)
            .expect("the dialog offers the CLI providers only");
          name.set(suggestions[index].clone());
        }
        picked.set(Picked(None));
      });
    }
    let saved = self.context.saved.clone();
    let save = save_action(ctx);
    ctx.watch(&save.state(), move |state| {
      if let (false, Some(Saved(id))) = (state.is_pending(), state.data) {
        saved(id);
      }
    });
    let picked = self.picked.clone();
    let pick = pick_action(ctx);
    ctx.watch(&pick.state(), move |state| {
      if let Some(PickedFile(Some(path))) = &state.data {
        picked.set(Picked(Some(path.clone())));
      }
    });
  }

  /// The program to check (chosen, or the newest found on the `PATH`) and what its check found.
  fn program(&self, ctx: &mut Ctx) -> Program {
    let provider = self.provider.get().0;
    let detect = ctx.future(provider, |provider| async move {
      background::run(move || Detected(probe::detect_agent(provider))).await
    });
    let detected = detect.state().get();
    let found = detected.data.clone().unwrap_or_default().0;
    let executable = self.picked.get().0.or_else(|| found.first().cloned());
    let deps = (provider, executable.clone(), self.attempt.get());
    let (activity, project) = (Activity::of(ctx), self.context.project.clone());
    let check = ctx.future(deps, move |(provider, executable, _)| {
      let (activity, spec) = (activity.clone(), operations::cli_check(&project, provider));
      async move {
        let (Some(executable), Some(spec)) = (executable, spec) else {
          return Ok(None);
        };
        let check = background::run(move || {
          let check = probe::check(provider, &executable)?;
          Some(Checked {
            provider,
            executable,
            check,
          })
        });
        activity.track(spec, check).await
      }
    });
    let checked = check.state().get();
    let pending = checked.is_pending();
    let current = checked
      .data
      .flatten()
      .filter(|checked| checked.provider == provider && Some(&checked.executable) == executable.as_ref());
    let verification = match (&executable, current, checked.error) {
      (Some(_), Some(checked), _) if !pending => Verification::Done(checked.check),
      (Some(_), None, Some(error)) if !pending => Verification::Failed(error),
      (Some(_), ..) => Verification::Checking,
      (None, ..) if detected.is_pending() => Verification::Detecting,
      (None, ..) => Verification::NotFound,
    };
    Program {
      provider,
      executable,
      found: found.len(),
      verification,
    }
  }

  /// What Save connection stores, once the name is valid and the program is the expected CLI.
  fn draft(&self, program: &Program) -> Result<ConnectionDraft, Message> {
    let name = form::connection_name(&self.name.get(), &self.context.taken)?;
    let (Some(executable), Verification::Done(check)) = (&program.executable, &program.verification) else {
      return Err(Message::new("connections.dialog.waitForCheck"));
    };
    if !check.result.is_verified() {
      return Err(Message::new("errors.connectionNotVerified"));
    }
    Ok(ConnectionDraft {
      id: match &self.context.mode {
        DialogMode::EditAgent(connection) => Some(connection.id()),
        _ => None,
      },
      name,
      provider: program.provider,
      executable: executable.clone(),
      check: check.clone(),
    })
  }

  /// Stores the draft; `None` until it can be saved, and while it is being saved.
  fn save(&self, save: &SaveAction, draft: Option<&ConnectionDraft>) -> Submit {
    if save.state().get_untracked().is_pending() {
      return None;
    }
    let (save, repo_root, draft) = (save.clone(), self.context.project.repo_root.clone(), draft?.clone());
    Some(Arc::new(move || save.run((repo_root.clone(), draft.clone()))))
  }
}

/// The provider and the project, e.g. "Codex · Kontur", as the design names connections.
/// Numbered ("Codex · Kontur 2") when the plain name is taken.
fn suggested_name(ctx: &Ctx, context: &DialogContext, provider: AgentProvider) -> String {
  let short = ctx.t(short_key(provider));
  let project = context.project.project.name().as_str();
  dialog_frame::suggested_name(
    ctx,
    context,
    "connections.dialog.suggestedName",
    "connections.dialog.suggestedNameNumbered",
    &[("provider", &*short), ("project", project)],
  )
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let project = ctx
    .use_context::<DialogContext>()
    .expect("the connections screen provides the dialog context")
    .project;
  ctx.future_action(move |(repo_root, draft): (PathBuf, ConnectionDraft)| {
    let spec = operations::connection_save(&project, draft.name.as_str());
    let activity = activity.clone();
    async move {
      let save = async move {
        let saved = background::run(move || save_in(&repo_root, draft)).await?;
        saved.map(Saved).map_err(|error| {
          tracing::error!(%error, "cannot save the connection");
          error.message()
        })
      };
      activity.track(spec, save).await
    }
  })
}

pub(super) fn pick_action(ctx: &mut Ctx) -> PickAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |title: Arc<str>| {
    let activity = activity.clone();
    async move {
      // Listed while the dialog is open; closing it without a file leaves no event.
      let running = activity.start(operations::program_pick());
      let picked = background::run(move || PickedFile(rfd::FileDialog::new().set_title(&*title).pick_file())).await?;
      if picked.0.is_some() {
        running.finish();
      }
      Ok(picked)
    }
  })
}
