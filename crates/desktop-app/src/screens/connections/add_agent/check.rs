//! The Program row of Add provider (`Program` `lzSGF` in `10 · Add provider`): the copy found on the search path (a
//! choice when there are several), whether it was found in the row's hint line, *Check* beside it, and under it what
//! checking found in its tone (`Note` `mvJ49`); a CLI that is not signed in shows the command that signs it in, to run
//! in a terminal. Only a copy on the search path, or the file chosen on the start page, can be chosen.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::{Ctx, FutureState},
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{AgentProvider, CheckResult, ConnectionCheck, SignIn};
use orchester_workspace::{connections::data::sign_in_command, messages::Message};

use super::{Copies, Found};
use crate::{
  screens::{
    connections::words::provider_key,
    project_settings::kit::{FormField, Tone},
  },
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::CodeBox,
    field::{FieldInput, SelectField},
    icon::Icon,
    message::message_text,
  },
};

/// Where finding and checking the program stand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Checked {
  /// Looking for the program on the search path.
  Detecting,
  /// No copy is on the search path.
  NoProgram,
  Checking,
  Done(ConnectionCheck),
  /// Finding or checking the program did not finish.
  Failed(Message),
}

impl Checked {
  pub(super) fn of(
    found: &FutureState<Copies, Message>,
    program: Option<&PathBuf>,
    checked: &FutureState<Found, Message>,
  ) -> Self {
    if found.is_pending() {
      return Self::Detecting;
    }
    if let Some(error) = found.error.clone().filter(|_| found.is_rejected()) {
      return Self::Failed(error);
    }
    if program.is_none() {
      return Self::NoProgram;
    }
    if checked.is_pending() {
      return Self::Checking;
    }
    match (&checked.data, &checked.error) {
      (_, Some(error)) if checked.is_rejected() => Self::Failed(error.clone()),
      (Some(Found(Some(check))), _) => Self::Done(check.clone()),
      _ => Self::Checking,
    }
  }

  /// The check that recognized the CLI, which the connection is saved with.
  pub(super) fn check(&self) -> Option<&ConnectionCheck> {
    match self {
      Self::Done(check) if check.result.is_verified() => Some(check),
      _ => None,
    }
  }

  /// Why the connection cannot be saved yet, as finding and checking stand.
  pub(super) fn save_reason(&self, ctx: &Ctx) -> Option<Arc<str>> {
    match self {
      Self::Detecting | Self::Checking => Some(ctx.t("connections.dialog.waitForCheck")),
      Self::NoProgram => Some(ctx.t("connections.dialog.reasons.noProgram")),
      Self::Failed(error) => Some(message_text(ctx, error)),
      Self::Done(check) if !check.result.is_verified() => Some(ctx.t("connections.dialog.reasons.notRecognized")),
      Self::Done(_) => None,
    }
  }
}

/// The program found on the search path, or a choice of the copies found, with *Check* beside it and what checking
/// found under it. On an API provider (`on_provider`) the CLI's own sign-in is not used, so a CLI found reads Found
/// whatever its sign-in (`virfc`).
pub(super) fn program_row(
  ctx: &mut Ctx,
  (provider, on_provider): (AgentProvider, bool),
  (found, chosen): (&FutureState<Copies, Message>, &Signal<String>),
  (state, recheck): (&Checked, &Signal<u64>),
) -> Element {
  let program = provider.program_name().unwrap_or_default();
  let copies: &[PathBuf] = found.data.as_ref().map_or(&[], |copies| copies.0.copies.as_slice());
  let label = ctx.t("connections.dialog.program");
  let field: Element = match copies {
    [] | [_] => FieldInput::new("connection-program", label.clone(), chosen.clone())
      .placeholder(ctx.t("connections.dialog.notFoundOnPath"))
      .mono()
      .locked()
      .render(ctx),
    copies => {
      let options = copies
        .iter()
        .map(|copy| SelectOption::new(copy.display().to_string(), copy.display().to_string()))
        .collect();
      SelectField::new("connection-program", label.clone(), chosen.clone(), options)
        .mono()
        .render(ctx)
    }
  };
  let busy = matches!(state, Checked::Detecting | Checked::Checking);
  let again = ActionButton::new(
    "connection-recheck",
    ButtonKind::Secondary,
    ctx.t("connections.dialog.check"),
  )
  .icon(Icon::RefreshCw)
  .in_form();
  let again = if busy {
    again.disabled()
  } else {
    let recheck = recheck.clone();
    again.on_click(move || recheck.update(|count| *count += 1))
  };
  let on_search_path = found.data.as_ref().map_or(0, |copies| copies.0.on_search_path);
  let (hint, tone) = match (copies.len(), on_search_path) {
    _ if found.is_pending() => (ctx.t("connections.dialog.detecting"), Tone::Muted),
    (0, _) => (
      ctx.t_args("connections.dialog.programMissing", [("program", program)]),
      Tone::Warning,
    ),
    (_, 0) => (ctx.t("connections.dialog.programChosen"), Tone::Success),
    (1, _) => (ctx.t("connections.dialog.programFoundOne"), Tone::Success),
    (_, count) => (
      ctx.t_args("connections.dialog.programFoundMany", [("count", count.to_string())]),
      Tone::Muted,
    ),
  };
  let control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(field))
        .child(again),
    )
    .child(check_line(ctx, provider, state, on_provider))
    .with_children(sign_in(ctx, provider, state, on_provider));
  FormField::new("connection-program", label)
    .toned(hint, tone)
    .render(ctx, control)
}

/// What checking found, one line in its tone under the program (`connection-check`, its words
/// `connection-check-detail`, its state word for tools).
fn check_line(ctx: &Ctx, provider: AgentProvider, state: &Checked, on_provider: bool) -> Element {
  let (tone, word, detail) = shown_on(ctx, provider, state, on_provider);
  Row::new()
    .id("connection-check")
    .width(Dimension::Pct(100.0))
    .describe("state", word)
    .child(
      Text::new(&detail)
        .id("connection-check-detail")
        .variant(ExtraTypography::Note.role())
        .color(tone.color())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// The command that signs the CLI in, to run in a terminal, while it is not signed in and runs on its own sign-in.
fn sign_in(ctx: &mut Ctx, provider: AgentProvider, state: &Checked, on_provider: bool) -> Option<Element> {
  let signed_out = matches!(
    state,
    Checked::Done(ConnectionCheck {
      result: CheckResult::Available {
        sign_in: SignIn::SignedOut,
        ..
      },
      ..
    })
  );
  if !signed_out || on_provider {
    return None;
  }
  let command = sign_in_command(provider)?;
  Some(
    CodeBox::new(
      "connection-sign-in-command",
      ctx.t("connections.dialog.runToSignIn"),
      command,
    )
    .render(ctx),
  )
}

/// The check line's tone, its state word and its sentence: on an API provider a CLI found reads Found whatever its own
/// sign-in (`virfc`).
fn shown_on(ctx: &Ctx, provider: AgentProvider, state: &Checked, on_provider: bool) -> (Tone, Arc<str>, Arc<str>) {
  let program = provider.program_name().unwrap_or_default();
  match state {
    Checked::Done(ConnectionCheck {
      result: CheckResult::Available { version, .. },
      ..
    }) if on_provider => (
      Tone::Success,
      ctx.t("connections.apiProvider.found"),
      ctx.t_args(
        "connections.apiProvider.checkFound",
        [("program", program), ("version", version.as_str())],
      ),
    ),
    _ => shown(ctx, (program, &ctx.t(provider_key(provider))), state),
  }
}

/// `program` is the CLI's program (`claude`), `cli` its name (*Claude Code CLI*).
fn shown(ctx: &Ctx, (program, cli): (&str, &str), state: &Checked) -> (Tone, Arc<str>, Arc<str>) {
  let line = |key: &str, version: &str| ctx.t_args(key, [("program", program), ("version", version)]);
  match state {
    Checked::Detecting => (
      Tone::Muted,
      ctx.t("connections.dialog.detectingWord"),
      ctx.t("connections.dialog.detecting"),
    ),
    Checked::Checking => (
      Tone::Muted,
      ctx.t("connections.status.checking"),
      ctx.t("connections.dialog.checking"),
    ),
    Checked::NoProgram => (
      Tone::Muted,
      ctx.t("connections.status.notChecked"),
      ctx.t("connections.dialog.noProgramToCheck"),
    ),
    Checked::Failed(error) => (
      Tone::Danger,
      ctx.t("connections.status.couldNotRun"),
      message_text(ctx, error),
    ),
    Checked::Done(check) => match &check.result {
      CheckResult::Available { version, sign_in, .. } => match sign_in {
        SignIn::SignedIn => (
          Tone::Success,
          ctx.t("connections.dialog.signedIn"),
          line("connections.dialog.verifiedSignedIn", version),
        ),
        SignIn::SignedOut => (
          Tone::Warning,
          ctx.t("connections.dialog.notSignedIn"),
          line("connections.dialog.verifiedSignedOut", version),
        ),
        SignIn::Unknown => (
          Tone::Secondary,
          ctx.t("connections.unknown"),
          line("connections.dialog.verifiedSignInUnknown", version),
        ),
      },
      CheckResult::ExecutableMissing => (
        Tone::Danger,
        ctx.t("connections.status.notFound"),
        ctx.t("connections.detail.executableMissing"),
      ),
      CheckResult::NotRecognized => (
        Tone::Danger,
        ctx.t("connections.status.notRecognized"),
        ctx.t_args("connections.dialog.notRecognized", [("program", cli)]),
      ),
      _ => (
        Tone::Danger,
        ctx.t("connections.status.couldNotRun"),
        ctx.t("connections.detail.couldNotRun"),
      ),
    },
  }
}
