//! The Program and Check rows of Add connection (`Row · Program` `f02h67`, `Row · Check` `FX1WS` and the
//! states of `l2kjVF`). Program shows the copy found on the search path (a choice when there are several);
//! Choose file… is not built in this app, so it stays locked with why. The check row says what checking
//! found, an icon and a word with a sentence after it (`Check row` `nL8Cy`), with Recheck at its end; a CLI
//! that is not signed in shows the command that signs it in, to run in a terminal.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::connection::{AgentProvider, CheckResult, ConnectionCheck, SignIn};
use orchester_workspace::{connections::data::sign_in_command, messages::Message};

use super::{Copies, Found};
use crate::{
  screens::connections::words::provider_key,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::CodeBox,
    field::{FieldInput, SelectField},
    form_section::{Label, Place, stacked_row},
    icon::{Icon, icon},
    message::message_text,
    unavailable::reason_line,
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

/// The program found on the search path, or a choice of the copies found; Choose file… locked.
pub(super) fn program_row(
  ctx: &mut Ctx,
  provider: AgentProvider,
  (found, chosen, shown): (&FutureState<Copies, Message>, &Signal<String>, &Signal<String>),
) -> Element {
  let program = provider.program_name().unwrap_or_default();
  let copies: &[PathBuf] = found.data.as_ref().map_or(&[], |copies| copies.0.as_slice());
  let label = ctx.t("connections.dialog.program");
  let control: Element = match copies {
    [] | [_] => {
      let path = copies
        .first()
        .map(|copy| copy.display().to_string())
        .unwrap_or_default();
      if shown.get_untracked() != path {
        shown.set(path);
      }
      FieldInput::new("connection-program", label.clone(), shown.clone())
        .placeholder(ctx.t("connections.dialog.notFoundOnPath"))
        .mono()
        .locked()
        .render(ctx)
    }
    copies => {
      let options = copies
        .iter()
        .map(|copy| SelectOption::new(copy.display().to_string(), copy.display().to_string()))
        .collect();
      if chosen.get_untracked().is_empty() {
        chosen.set(copies[0].display().to_string());
      }
      SelectField::new("connection-program", label.clone(), chosen.clone(), options).render(ctx)
    }
  };
  let choose_reason = ctx.t("connections.dialog.chooseFileUnavailable");
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Column::new().flex(1.0).child(control))
    .child(
      ActionButton::new(
        "connection-choose-file",
        ButtonKind::Secondary,
        ctx.t("connections.dialog.chooseFile"),
      )
      .icon(Icon::FolderOpen)
      .unavailable(choose_reason.clone()),
    );
  let (hint, color) = match copies.len() {
    _ if found.is_pending() => (ctx.t("connections.dialog.detecting"), PaletteColor::TextMuted),
    0 => (
      ctx.t_args("connections.dialog.programMissing", [("program", program)]),
      PaletteColor::Warning,
    ),
    1 => (ctx.t("connections.dialog.programFoundOne"), PaletteColor::TextMuted),
    count => (
      ctx.t_args("connections.dialog.programFoundMany", [("count", count.to_string())]),
      PaletteColor::TextMuted,
    ),
  };
  let control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(line)
    .child(
      Text::new(&hint)
        .id("connection-program-hint")
        .variant(ExtraTypography::Note.role())
        .color(color),
    )
    .child(reason_line("connection-choose-file-reason", &choose_reason));
  let description = ctx.t_args("connections.dialog.programHint", [("program", program)]);
  stacked_row(
    Label::new("connection-program", label).description(description),
    control,
    Place::Within,
  )
}

/// The check row, and the sign-in command under it while the CLI is not signed in.
pub(super) fn check_row(ctx: &mut Ctx, provider: AgentProvider, state: &Checked, recheck: &Signal<u64>) -> Element {
  let cli = ctx.t(provider_key(provider));
  let (glyph, tone, word, detail) = shown(ctx, &cli, state);
  let busy = matches!(state, Checked::Detecting | Checked::Checking);
  let again =
    ActionButton::new("connection-recheck", ButtonKind::Ghost, ctx.t("connections.recheck")).icon(Icon::RefreshCw);
  let again = if busy {
    again.disabled()
  } else {
    let recheck = recheck.clone();
    again.on_click(move || recheck.update(|count| *count += 1))
  };
  let word_color = match tone {
    PaletteColor::Warning | PaletteColor::Danger => tone.clone(),
    _ => PaletteColor::TextSecondary,
  };
  let row = Row::new()
    .id("connection-check")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Sm)
        .left(SpacingSize::Md)
        .right(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .describe("state", word.clone())
    .child(
      Row::new()
        .flex_shrink(0.0)
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, tone))
        .child(
          Text::new(&word)
            .variant(TypographyStyle::Label)
            .color(word_color)
            .nowrap(),
        ),
    )
    .child(
      Text::new(&detail)
        .id("connection-check-detail")
        .variant(ExtraTypography::Prose.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(again);
  let command = match state {
    Checked::Done(ConnectionCheck {
      result: CheckResult::Available {
        sign_in: SignIn::SignedOut,
        ..
      },
      ..
    }) => sign_in_command(provider).map(|command| {
      CodeBox::new(
        "connection-sign-in-command",
        ctx.t("connections.dialog.runToSignIn"),
        command,
      )
      .render(ctx)
    }),
    _ => None,
  };
  let control = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(row)
    .with_children(command);
  stacked_row(
    Label::new("connection-check", ctx.t("connections.dialog.check")),
    control,
    Place::Last,
  )
}

/// The check row's icon, its color, word and sentence.
fn shown(ctx: &Ctx, cli: &str, state: &Checked) -> (Icon, PaletteColor, Arc<str>, Arc<str>) {
  let line = |key: &str, version: &str| ctx.t_args(key, [("program", cli), ("version", version)]);
  match state {
    Checked::Detecting => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      ctx.t("connections.dialog.detectingWord"),
      ctx.t("connections.dialog.detecting"),
    ),
    Checked::Checking => (
      Icon::LoaderCircle,
      PaletteColor::TextSecondary,
      ctx.t("connections.status.checking"),
      ctx.t("connections.dialog.checking"),
    ),
    Checked::NoProgram => (
      Icon::Circle,
      PaletteColor::TextMuted,
      ctx.t("connections.status.notChecked"),
      ctx.t("connections.dialog.noProgramToCheck"),
    ),
    Checked::Failed(error) => (
      Icon::CircleAlert,
      PaletteColor::Danger,
      ctx.t("connections.status.couldNotRun"),
      message_text(ctx, error),
    ),
    Checked::Done(check) => match &check.result {
      CheckResult::Available { version, sign_in, .. } => match sign_in {
        SignIn::SignedIn => (
          Icon::CircleCheck,
          PaletteColor::Success,
          ctx.t("connections.dialog.signedIn"),
          line("connections.dialog.verifiedSignedIn", version),
        ),
        SignIn::SignedOut => (
          Icon::CircleAlert,
          PaletteColor::Warning,
          ctx.t("connections.dialog.notSignedIn"),
          line("connections.dialog.verifiedSignedOut", version),
        ),
        SignIn::Unknown => (
          Icon::CircleHelp,
          PaletteColor::TextSecondary,
          ctx.t("connections.unknown"),
          line("connections.dialog.verifiedSignInUnknown", version),
        ),
      },
      CheckResult::ExecutableMissing => (
        Icon::CircleAlert,
        PaletteColor::Danger,
        ctx.t("connections.status.notFound"),
        ctx.t("connections.detail.executableMissing"),
      ),
      CheckResult::NotRecognized => (
        Icon::CircleAlert,
        PaletteColor::Danger,
        ctx.t("connections.status.notRecognized"),
        line("connections.dialog.notRecognized", ""),
      ),
      _ => (
        Icon::CircleAlert,
        PaletteColor::Danger,
        ctx.t("connections.status.couldNotRun"),
        ctx.t("connections.detail.couldNotRun"),
      ),
    },
  }
}
