//! Add secret (07.02 `tpF7R`, dialog `E213dn`) and Replace value: one dialog in two modes. The
//! value is typed into a masked field (lurq masks it on screen and in the MCP tree), taken out of
//! the field when saving, so the field is empty again at once, and handed to the keychain; it is
//! never shown again. A name already used in the chosen environment keeps Save off and links the
//! existing secret (07.A `m9Dhp`). The fields are laid out in `fields`.

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
  id::SecretId,
  secret::{self, Environment, NewSecret, Secret, SecretKind, SecretName, SecretNameError, SecretValue},
};
use zeroize::Zeroize;

use crate::{
  activity::{Activity, OperationSpec, operations},
  background,
  messages::{self, Message},
  secrets::{
    Vault,
    data::{add_in, replace_in},
  },
  ui::form::Submit,
  workspace::OpenProject,
};

/// What the dialog is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DialogPurpose {
  Add,
  /// A new value for this secret (rotation).
  Replace(Secret),
}

/// Provided by the secrets screen before it mounts the dialog.
#[derive(Clone)]
pub struct SecretDialogContext {
  pub project: OpenProject,
  pub vault: Vault,
  pub purpose: DialogPurpose,
  /// The project's secrets, to find a name that is already taken.
  pub secrets: Arc<Vec<Secret>>,
  /// The environment the list shows, if one: a new secret starts in it.
  pub environment: Option<Environment>,
  pub close: Arc<dyn Fn() + Send + Sync>,
  pub saved: Arc<dyn Fn(SecretId) + Send + Sync>,
  /// Shows an existing secret, the one whose name was entered again.
  pub show: Arc<dyn Fn(SecretId) + Send + Sync>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindChoice(pub SecretKind);

impl lurq::app::component::DevtoolsInspectable for KindChoice {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnvironmentChoice(pub Environment);

impl lurq::app::component::DevtoolsInspectable for EnvironmentChoice {}

/// The secret that was added, or whose value was replaced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Saved(SecretId);

impl lurq::app::component::DevtoolsInspectable for Saved {}

/// What saving does, besides storing the value.
#[derive(Clone)]
enum Save {
  Add(NewSecret),
  Replace(SecretId),
}

struct SaveArgs {
  repo_root: PathBuf,
  vault: Vault,
  save: Save,
  value: SecretValue,
  spec: OperationSpec,
}

type SaveAction = FutureAction<SaveArgs, Saved, Message>;

/// What the entered name allows.
pub(super) enum NameCheck {
  /// Nothing entered yet.
  Empty,
  Invalid(Message),
  /// Another secret has the name in the chosen environment.
  Taken(Secret),
  Free(SecretName),
}

pub struct SecretDialog {
  pub(super) context: SecretDialogContext,
  pub(super) name: Signal<String>,
  pub(super) kind: Signal<KindChoice>,
  pub(super) environment: Signal<EnvironmentChoice>,
  /// The value being typed. Made outside `ctx.signal` on purpose: lurq's DevTools record every
  /// component signal with its history, and the value must not appear there.
  pub(super) value: Signal<String>,
  /// The dialog, focused when it opens so that Tab starts at its first control; the design shows
  /// no field focused (`E213dn`).
  frame: ElementRef,
}

impl Component for SecretDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<SecretDialogContext>()
      .expect("the secrets screen provides the dialog context");
    let (kind, environment) = match &context.purpose {
      DialogPurpose::Replace(secret) => (secret.kind(), secret.environment()),
      DialogPurpose::Add => (SecretKind::ALL[0], context.environment.unwrap_or(Environment::ALL[0])),
    };
    let save = save_action(ctx);
    let saved = context.saved.clone();
    ctx.watch(&save.state(), move |state| {
      if let (false, Some(Saved(id))) = (state.is_pending(), state.data) {
        saved(id);
      }
    });
    let dialog = Self {
      name: ctx.signal(String::new()),
      kind: ctx.signal(KindChoice(kind)),
      environment: ctx.signal(EnvironmentChoice(environment)),
      value: Signal::new(String::new()),
      frame: ElementRef::new(),
      context,
    };
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let save = save_action(ctx);
    let check = self.check_name();
    let state = save.state().get();
    let saving = state.is_pending();
    let submit = if saving { None } else { self.submit(&save, &check) };
    let error = state.error.filter(|_| !saving);
    self.frame(ctx, &self.frame, &check, (submit, saving, error))
  }
}

impl SecretDialog {
  /// The entered name against the rules and the project's other secrets; a replaced secret keeps
  /// its name.
  pub(super) fn check_name(&self) -> NameCheck {
    if let DialogPurpose::Replace(secret) = &self.context.purpose {
      return NameCheck::Free(secret.name().clone());
    }
    let entered = self.name.get();
    if entered.trim().is_empty() {
      return NameCheck::Empty;
    }
    let name = match SecretName::new(&entered) {
      Ok(name) => name,
      Err(error) => return NameCheck::Invalid(name_error(&error)),
    };
    let environment = self.environment.get().0;
    match secret::existing(&self.context.secrets, &name, environment) {
      Some(taken) => NameCheck::Taken(taken.clone()),
      None => NameCheck::Free(name),
    }
  }

  /// Saves the value; `None` while the name or the value cannot be saved.
  fn submit(&self, save: &SaveAction, check: &NameCheck) -> Submit {
    let NameCheck::Free(name) = check else {
      return None;
    };
    if self.value.with(|value| value.trim().is_empty()) {
      return None;
    }
    let project = &self.context.project;
    let (save_kind, spec) = match &self.context.purpose {
      DialogPurpose::Add => {
        let new = NewSecret {
          name: name.clone(),
          kind: self.kind.get().0,
          environment: self.environment.get().0,
        };
        (Save::Add(new), operations::secret_add(project, name.as_str()))
      }
      DialogPurpose::Replace(secret) => (
        Save::Replace(secret.id()),
        operations::secret_replace(project, name.as_str()),
      ),
    };
    let (save, value, vault, repo_root) = (
      save.clone(),
      self.value.clone(),
      self.context.vault.clone(),
      project.repo_root.clone(),
    );
    Some(Arc::new(move || {
      let mut entered = String::new();
      value.update(|text| std::mem::swap(text, &mut entered));
      // Save is offered only while the field holds a value; a blank one is wiped and dropped.
      let Ok(value) = SecretValue::new(entered) else {
        return;
      };
      save.run(SaveArgs {
        repo_root: repo_root.clone(),
        vault: vault.clone(),
        save: save_kind.clone(),
        value,
        spec: spec.clone(),
      });
    }))
  }

  /// Closes the dialog; a value typed but not saved is wiped first.
  pub(super) fn close(&self) -> impl Fn() + Send + Sync + 'static {
    let (value, close) = (self.value.clone(), self.context.close.clone());
    move || {
      value.update(|text| text.zeroize());
      close();
    }
  }
}

fn name_error(error: &SecretNameError) -> Message {
  match error {
    // A blank name shows nothing: Save is simply off.
    SecretNameError::Blank | SecretNameError::Invalid => Message::new("secrets.dialog.nameInvalid"),
    SecretNameError::TooLong { .. } => {
      Message::new("secrets.dialog.nameTooLong").with("max", SecretName::MAX_CHARS.to_string())
    }
  }
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  let activity = Activity::of(ctx);
  ctx.future_action(move |args: SaveArgs| {
    let activity = activity.clone();
    let spec = args.spec.clone();
    async move {
      let save = async move {
        let saved = background::run(move || {
          let SaveArgs {
            repo_root,
            vault,
            save,
            value,
            ..
          } = args;
          match save {
            Save::Add(new) => add_in(&repo_root, vault.get(), new, &value),
            Save::Replace(id) => replace_in(&repo_root, vault.get(), id, &value).map(|()| id),
          }
        })
        .await?;
        saved.map(Saved).map_err(|error| {
          tracing::error!(%error, "cannot save the secret");
          messages::secret_error(&error)
        })
      };
      activity.track(spec, save).await
    }
  })
}
