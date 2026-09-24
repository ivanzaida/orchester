//! Add connection (04.02 `h6CmJ`, dialog `IAN4P`), also used to edit one. The provider's CLI
//! is looked up on the `PATH` (or chosen as a file) and checked as soon as it is known; the
//! connection can be saved only when the check found the expected CLI. Checking runs only
//! the CLI's read-only commands (`orchester_core::connection`). The fields are in `form`.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, ConnectionCheck},
  id::ConnectionId,
};

use super::{DialogContext, DialogMode, form, status::short_key};
use crate::{
  background,
  connections::{
    data::{ConnectionDraft, save_in},
    probe,
  },
  messages::Message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// Dialog, header, action bar and button sizes (`IAN4P`, `osMan`, `InHni`, `I3629f`).
const DIALOG_WIDTH: f32 = 800.0;
const DIALOG_HEIGHT: f32 = 832.0;
const HEADER_HEIGHT: f32 = 82.0;
const ACTIONS_HEIGHT: f32 = 76.0;
pub(super) const BUTTON_HEIGHT: f32 = 36.0;

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
pub(super) struct PickedFile(Option<PathBuf>);

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
}

impl Component for ConnectionDialog {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let context = ctx
      .use_context::<DialogContext>()
      .expect("the connections screen provides the dialog context");
    let (provider, name, picked) = match &context.mode {
      DialogMode::Edit(connection) => (
        connection.provider(),
        connection.name().as_str().to_owned(),
        Some(connection.executable().to_path_buf()),
      ),
      DialogMode::Add | DialogMode::Closed => {
        let provider = AgentProvider::CodexCli;
        (provider, suggested_name(ctx, &context, provider), None)
      }
    };
    let dialog = Self {
      provider: ctx.signal(ProviderChoice(provider)),
      name: ctx.signal(name),
      picked: ctx.signal(Picked(picked)),
      attempt: ctx.signal(0),
      context,
    };
    dialog.watch(ctx);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Futures are matched by call order: the actions come first, as in `create`.
    let save = save_action(ctx);
    let pick = pick_action(ctx);
    let program = self.program(ctx);
    let draft = self.draft(&program);
    let form = self.form(ctx, &program, &pick);
    let dialog = Column::new()
      .id("connection-dialog")
      .width(Dimension::Px(DIALOG_WIDTH))
      .height(Dimension::Px(DIALOG_HEIGHT))
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(self.header(ctx))
      .child(ScrollVertical::new(form).width(Dimension::Pct(100.0)).flex(1.0))
      .child(self.actions(ctx, &save, draft));
    backdrop(dialog)
  }
}

impl ConnectionDialog {
  /// A new provider gets its own program and, unless the user renamed the connection, its own
  /// suggested name. A saved connection closes the dialog; a chosen file replaces the found one.
  fn watch(&self, ctx: &mut Ctx) {
    if self.context.mode == DialogMode::Add {
      let suggestions = AgentProvider::ALL.map(|provider| suggested_name(ctx, &self.context, provider));
      let (name, picked) = (self.name.clone(), self.picked.clone());
      ctx.watch(&self.provider, move |choice| {
        if suggestions.contains(&name.get_untracked()) {
          let index = AgentProvider::ALL
            .iter()
            .position(|provider| *provider == choice.0)
            .expect("every provider is in ALL");
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

  /// The program to check (chosen, or the first found on the `PATH`) and what its check found.
  fn program(&self, ctx: &mut Ctx) -> Program {
    let provider = self.provider.get().0;
    let detect = ctx.future(provider, |provider| async move {
      background::run(move || Detected(probe::detect(provider))).await
    });
    let detected = detect.state().get();
    let found = detected.data.clone().unwrap_or_default().0;
    let executable = self.picked.get().0.or_else(|| found.first().cloned());
    let deps = (provider, executable.clone(), self.attempt.get());
    let check = ctx.future(deps, |(provider, executable, _)| async move {
      let Some(executable) = executable else {
        return Ok(None);
      };
      background::run(move || {
        let check = probe::check(provider, &executable);
        Some(Checked {
          provider,
          executable,
          check,
        })
      })
      .await
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
        DialogMode::Edit(connection) => Some(connection.id()),
        DialogMode::Add | DialogMode::Closed => None,
      },
      name,
      provider: program.provider,
      executable: executable.clone(),
      check: check.clone(),
    })
  }

  /// Title, the owning project and Close (`osMan`).
  fn header(&self, ctx: &Ctx) -> Element {
    let title = match self.context.mode {
      DialogMode::Edit(_) => "connections.dialog.editTitle",
      DialogMode::Add | DialogMode::Closed => "connections.dialog.addTitle",
    };
    let close = self.context.close.clone();
    let owner = ctx.t_args(
      "connections.dialog.owner",
      [("project", self.context.project.project.name().as_str())],
    );
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(HEADER_HEIGHT))
      .padding_horizontal(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(
        Column::new()
          .flex(1.0)
          .spacing(SpacingSize::Sm)
          .child(Text::new(&ctx.t(title)).variant(ExtraTypography::DialogTitle.role()))
          .child(Text::new(&owner).variant(TypographyStyle::Caption)),
      )
      .child(
        ActionButton::new(ctx.t("common.close"))
          .icon(Icon::X)
          .id("connection-dialog-close")
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(move || close())
          .render(ctx),
      )
      .into()
  }

  /// Cancel and Save connection (`InHni`), with why saving is not possible yet.
  fn actions(&self, ctx: &Ctx, save: &SaveAction, draft: Result<ConnectionDraft, Message>) -> Element {
    let state = save.state().get();
    let saving = state.is_pending();
    let reason = match (&draft, state.error.filter(|_| !saving)) {
      (_, Some(error)) => Some(error),
      (Err(reason), None) => Some(reason.clone()),
      (Ok(_), None) => None,
    };
    let label = if saving {
      "connections.dialog.saving"
    } else {
      "connections.dialog.save"
    };
    let save_button = ActionButton::new(ctx.t(label))
      .primary()
      .icon(Icon::Check)
      .id("connection-save")
      .height(Dimension::Px(BUTTON_HEIGHT));
    let save_button = match draft {
      Ok(draft) if !saving => {
        let (save, repo_root) = (save.clone(), self.context.project.repo_root.clone());
        save_button.on_click(move || save.run((repo_root.clone(), draft.clone())))
      }
      // Enabled once the reason shown beside it is resolved.
      _ => save_button.on_click(|| {}).enabled(false),
    };
    let close = self.context.close.clone();
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ACTIONS_HEIGHT))
      .padding_horizontal(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .with_children(reason.map(|reason| save_reason(ctx, &reason)))
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new(ctx.t("common.cancel"))
          .icon(Icon::X)
          .id("connection-cancel")
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(move || close())
          .render(ctx),
      )
      .child(save_button.render(ctx))
      .into()
  }
}

fn save_reason(ctx: &Ctx, reason: &Message) -> Element {
  Row::new()
    .id("connection-save-reason")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(icon(Icon::AlertCircle, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&message_text(ctx, reason)).variant(TypographyStyle::Caption))
    .into()
}

/// The provider and the project, e.g. "Codex · Kontur", as the design names connections.
/// Numbered ("Codex · Kontur 2") when the plain name is taken.
fn suggested_name(ctx: &Ctx, context: &DialogContext, provider: AgentProvider) -> String {
  let (short, project) = (ctx.t(short_key(provider)), context.project.project.name().as_str());
  let taken = |name: &str| context.taken.iter().any(|taken| taken.as_str() == name);
  let plain = ctx.t_args(
    "connections.dialog.suggestedName",
    [("provider", &*short), ("project", project)],
  );
  if !taken(&plain) {
    return plain.to_string();
  }
  (2_usize..)
    .map(|number| {
      ctx
        .t_args(
          "connections.dialog.suggestedNameNumbered",
          [
            ("provider", &*short),
            ("project", project),
            ("number", number.to_string().as_str()),
          ],
        )
        .to_string()
    })
    .find(|name| !taken(name))
    .expect("only finitely many connection names are taken")
}

fn save_action(ctx: &mut Ctx) -> SaveAction {
  ctx.future_action(|(repo_root, draft): (PathBuf, ConnectionDraft)| async move {
    let saved = background::run(move || save_in(&repo_root, draft)).await?;
    saved.map(Saved).map_err(|error| {
      tracing::error!(%error, "cannot save the connection");
      error.message()
    })
  })
}

pub(super) fn pick_action(ctx: &mut Ctx) -> PickAction {
  ctx.future_action(|title: Arc<str>| async move {
    background::run(move || PickedFile(rfd::FileDialog::new().set_title(&*title).pick_file())).await
  })
}
