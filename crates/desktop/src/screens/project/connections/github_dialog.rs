//! Add GitHub connection (04.06 `D4mDB`, dialog `d8yPD5`), also used to edit one. `gh` is looked
//! up on the `PATH` (or chosen as a file) and checked for the host as soon as both are known;
//! the connection can be saved once the program answered as the GitHub CLI. Checking runs only
//! `gh`'s read-only commands (`orchester_core::connection::check_github`). The fields are in
//! `github_form`.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  core::{ElementRef, Signal},
  node::Element,
};
use orchester_core::{
  connection::{GH_PROGRAM, GitHubCheck, GitHubHost},
  id::ConnectionId,
};

use super::{
  DialogContext, DialogMode, KindChoice,
  dialog::{Picked, PickedFile, pick_action},
  dialog_frame::{self, SaveBar, dialog, save_reason_of},
  form,
};
use crate::{
  activity::{Activity, operations},
  background,
  connections::{
    github::{GitHubDraft, save_github_in},
    probe,
  },
  messages::Message,
  ui::form::Submit,
};

/// The host select (`lO1rv`): github.com, or a GitHub Enterprise host typed below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HostChoice {
  GitHubCom,
  Other,
}

impl lurq::app::component::DevtoolsInspectable for HostChoice {}

/// A check and what it checked, so a result is never shown for other inputs.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Checked {
  host: GitHubHost,
  executable: PathBuf,
  check: GitHubCheck,
}

impl lurq::app::component::DevtoolsInspectable for Checked {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(ConnectionId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// Copies of `gh` found on the `PATH`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Detected(Vec<PathBuf>);

impl lurq::app::component::DevtoolsInspectable for Detected {}

type SaveAction = FutureAction<(PathBuf, GitHubDraft), Saved, Message>;

/// What the dialog knows about the program.
pub(super) enum GhVerification {
  Detecting,
  NotFound,
  /// The host is not valid yet, so there is nothing to check.
  NoHost,
  Checking,
  /// The check could not run to the end (the background work stopped).
  Failed(Message),
  Done(GitHubCheck),
}

/// The program being set up and what is known about it.
pub(super) struct GhProgram {
  pub host: Result<GitHubHost, Message>,
  pub executable: Option<PathBuf>,
  /// Copies found on the `PATH`.
  pub found: usize,
  /// The program is a file the user chose, not the one found on the `PATH`.
  pub chosen: bool,
  pub verification: GhVerification,
}

pub struct GitHubDialog {
  pub(super) context: DialogContext,
  pub(super) name: Signal<String>,
  pub(super) host: Signal<HostChoice>,
  pub(super) other_host: Signal<String>,
  pub(super) picked: Signal<Picked>,
  pub(super) attempt: Signal<u64>,
  pub(super) kind: Signal<KindChoice>,
  frame: ElementRef,
}

impl Component for GitHubDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<DialogContext>()
      .expect("the connections screen provides the dialog context");
    let (name, host, other_host, picked) = match &context.mode {
      DialogMode::EditGitHub(connection) => {
        let host = connection.host().as_str();
        let choice = if host == GitHubHost::GITHUB_COM {
          HostChoice::GitHubCom
        } else {
          HostChoice::Other
        };
        (
          connection.name().as_str().to_owned(),
          choice,
          host.to_owned(),
          Some(connection.executable().to_path_buf()),
        )
      }
      _ => (
        suggested_name(ctx, &context, GitHubHost::GITHUB_COM),
        HostChoice::GitHubCom,
        String::new(),
        None,
      ),
    };
    let dialog = Self {
      name: ctx.signal(name),
      host: ctx.signal(host),
      other_host: ctx.signal(other_host),
      picked: ctx.signal(Picked(picked)),
      attempt: ctx.signal(0),
      kind: ctx.signal(KindChoice::GitHub),
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
      DialogMode::EditGitHub(_) => "github.dialog.editTitle",
      _ => "github.dialog.addTitle",
    };
    dialog(ctx, &self.frame, &ctx.t(title), &self.context, fields, bar)
  }
}

impl GitHubDialog {
  pub(super) const fn editing(&self) -> bool {
    matches!(self.context.mode, DialogMode::EditGitHub(_))
  }

  /// Switching the type back opens the agent dialog. Unless the user renamed the connection, a
  /// new host gets its own suggested name. A saved connection closes the dialog; a chosen file
  /// replaces the found one.
  fn watch(&self, ctx: &mut Ctx) {
    let retype = self.context.retype.clone();
    ctx.watch(&self.kind, move |kind| retype(*kind));
    if !self.editing() {
      self.follow_host(ctx);
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

  /// Keeps the suggested name in step with the host while the user has not renamed it. The
  /// watchers run without a context, so the names are made from the messages with their
  /// placeholders kept.
  fn follow_host(&self, ctx: &mut Ctx) {
    let keep = [("host", "{{host}}"), ("number", "{{number}}")];
    let plain = ctx.t_args("github.dialog.suggestedName", keep).to_string();
    let numbered = ctx.t_args("github.dialog.suggestedNameNumbered", keep).to_string();
    let taken = self.context.taken.clone();
    let suggest = move |host: &str| -> String {
      let is_taken = |name: &str| taken.iter().any(|taken| taken.as_str() == name);
      let plain = plain.replace("{{host}}", host);
      if !is_taken(&plain) {
        return plain;
      }
      (2_usize..)
        .map(|number| {
          numbered
            .replace("{{host}}", host)
            .replace("{{number}}", &number.to_string())
        })
        .find(|name| !is_taken(name))
        .expect("only finitely many connection names are taken")
    };
    let (name, host, other) = (self.name.clone(), self.host.clone(), self.other_host.clone());
    let last = ctx.signal(self.name.get_untracked());
    let update = move || {
      let current = match host.get_untracked() {
        HostChoice::GitHubCom => Ok(GitHubHost::github_com()),
        HostChoice::Other => GitHubHost::new(other.get_untracked().trim()),
      };
      // An entered host is named only once it is a valid one.
      let Ok(current) = current else {
        return;
      };
      if name.get_untracked() == last.get_untracked() {
        let next = suggest(current.as_str());
        name.set(next.clone());
        last.set(next);
      }
    };
    let on_other = update.clone();
    ctx.watch(&self.host, move |_| update());
    ctx.watch(&self.other_host, move |_| on_other());
  }

  /// The host of the connection: github.com, or the entered host if it is valid.
  pub(super) fn chosen_host(&self) -> Result<GitHubHost, Message> {
    if let DialogMode::EditGitHub(connection) = &self.context.mode {
      return Ok(connection.host().clone());
    }
    match self.host.get() {
      HostChoice::GitHubCom => Ok(GitHubHost::github_com()),
      HostChoice::Other => {
        let entered = self.other_host.get();
        if entered.trim().is_empty() {
          return Err(Message::new("errors.githubHostRequired"));
        }
        GitHubHost::new(entered.trim()).map_err(|_| Message::new("errors.githubHostInvalid"))
      }
    }
  }

  /// The program to check (chosen, or the first found on the `PATH`) and what its check found.
  fn program(&self, ctx: &mut Ctx) -> GhProgram {
    let detect = ctx.future((), |()| async move {
      background::run(move || Detected(probe::detect(GH_PROGRAM))).await
    });
    let detected = detect.state().get();
    let found = detected.data.clone().unwrap_or_default().0;
    let picked = self.picked.get().0;
    let chosen = picked.is_some();
    let executable = picked.or_else(|| found.first().cloned());
    let host = self.chosen_host();
    let deps = (host.clone().ok(), executable.clone(), self.attempt.get());
    let (activity, project) = (Activity::of(ctx), self.context.project.clone());
    let check = ctx.future(deps, move |(host, executable, _)| {
      let (activity, spec) = (activity.clone(), operations::gh_check(&project));
      async move {
        let (Some(host), Some(executable)) = (host, executable) else {
          return Ok(None);
        };
        let check = background::run(move || {
          let check = probe::check_github(&host, &executable);
          Some(Checked {
            host,
            executable,
            check,
          })
        });
        activity.track(spec, check).await
      }
    });
    let checked = check.state().get();
    let pending = checked.is_pending();
    let current = checked.data.flatten().filter(|checked| {
      Some(&checked.executable) == executable.as_ref() && host.as_ref().is_ok_and(|host| *host == checked.host)
    });
    let verification = match (&executable, &host, current, checked.error) {
      (Some(_), Err(_), ..) => GhVerification::NoHost,
      (Some(_), Ok(_), Some(checked), _) if !pending => GhVerification::Done(checked.check),
      (Some(_), Ok(_), None, Some(error)) if !pending => GhVerification::Failed(error),
      (Some(_), ..) => GhVerification::Checking,
      (None, ..) if detected.is_pending() => GhVerification::Detecting,
      (None, ..) => GhVerification::NotFound,
    };
    GhProgram {
      host,
      executable,
      found: found.len(),
      chosen,
      verification,
    }
  }

  /// What Save connection stores, once the name and host are valid and the program is `gh`.
  fn draft(&self, program: &GhProgram) -> Result<GitHubDraft, Message> {
    let name = form::connection_name(&self.name.get(), &self.context.taken)?;
    let host = program.host.clone()?;
    let (Some(executable), GhVerification::Done(check)) = (&program.executable, &program.verification) else {
      return Err(Message::new("connections.dialog.waitForCheck"));
    };
    if !check.result.is_verified() {
      return Err(Message::new("errors.githubNotVerified"));
    }
    Ok(GitHubDraft {
      id: match &self.context.mode {
        DialogMode::EditGitHub(connection) => Some(connection.id()),
        _ => None,
      },
      name,
      host,
      executable: executable.clone(),
      check: check.clone(),
    })
  }

  /// Stores the draft; `None` until it can be saved, and while it is being saved.
  fn save(&self, save: &SaveAction, draft: Option<&GitHubDraft>) -> Submit {
    if save.state().get_untracked().is_pending() {
      return None;
    }
    let (save, repo_root, draft) = (save.clone(), self.context.project.repo_root.clone(), draft?.clone());
    Some(Arc::new(move || save.run((repo_root.clone(), draft.clone()))))
  }
}

/// "GitHub · github.com", numbered when the plain name is taken.
fn suggested_name(ctx: &Ctx, context: &DialogContext, host: &str) -> String {
  dialog_frame::suggested_name(
    ctx,
    context,
    "github.dialog.suggestedName",
    "github.dialog.suggestedNameNumbered",
    &[("host", host)],
  )
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  let project = ctx
    .use_context::<DialogContext>()
    .expect("the connections screen provides the dialog context")
    .project;
  ctx.future_action(move |(repo_root, draft): (PathBuf, GitHubDraft)| {
    let spec = operations::connection_save(&project, draft.name.as_str());
    let activity = activity.clone();
    async move {
      let save = async move {
        let saved = background::run(move || save_github_in(&repo_root, draft)).await?;
        saved.map(Saved).map_err(|error| {
          tracing::error!(%error, "cannot save the GitHub connection");
          error.message()
        })
      };
      activity.track(spec, save).await
    }
  })
}
