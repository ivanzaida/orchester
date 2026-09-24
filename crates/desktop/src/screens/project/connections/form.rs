//! Fields of the connection dialog (`dCLMl`): name and type, provider and where it runs, the
//! program file and how it signs in, what the check found and the models it offers.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  connection::{AgentProvider, CheckResult, ConnectionCheck, ConnectionStatus},
  name::{Name, NameError},
};

use super::{
  DialogMode,
  dialog::{ConnectionDialog, PickAction, Program, ProviderChoice, Verification},
  inspector::model_chips,
  status::{CHECKING, program_key, status_label},
};
use crate::{
  display,
  messages::{self, Message},
  theme::{ExtraSpacing, ExtraTypography, SELECT_CHEVRON_SIZE},
  ui::{
    button::ActionButton,
    field::{FieldNote, ValueTone, field, select_trigger, text_input, value_box},
    icon::Icon,
    message_text,
    notice::Notice,
    status_label::status_label as label,
  },
};

/// Field and Recheck heights (`Iitmk`, `JWxsC`), and the longest program path shown in full.
const FIELD_HEIGHT: f32 = 40.0;
const RECHECK_HEIGHT: f32 = 30.0;
const PATH_CHARS: usize = 44;

impl ConnectionDialog {
  pub(super) fn form(&self, ctx: &Ctx, program: &Program, pick: &PickAction) -> Element {
    let program_name = ctx.t(program_key(program.provider));
    let program_file = self.program_field(ctx, &program_name, program, pick);
    let models = match &program.verification {
      Verification::Done(ConnectionCheck {
        result: CheckResult::Available { catalog, .. },
        ..
      }) => model_chips(ctx, catalog.models()),
      _ => Text::new(&ctx.t("connections.modelsAfterCheck"))
        .variant(TypographyStyle::Caption)
        .into(),
    };
    let project = self.context.project.project.name().as_str();
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Form.role())
      .child(pair(
        self.name_field(ctx),
        fixed(
          ctx,
          "connection-type",
          "connections.dialog.type",
          "connections.type.agentProvider",
        ),
      ))
      .child(pair(
        self.provider_field(ctx, program.provider),
        fixed(
          ctx,
          "connection-execution",
          "connections.property.execution",
          "connections.thisComputer",
        ),
      ))
      .child(pair(
        program_file,
        fixed(
          ctx,
          "connection-authentication",
          "connections.property.authentication",
          "connections.cliLogin",
        ),
      ))
      .child(Notice::new(
        Icon::ShieldCheck,
        ctx.t_args("connections.dialog.authTitle", [("program", &*program_name)]),
        ctx.t_args("connections.dialog.authDetail", [("project", project)]),
      ))
      .child(self.verification_row(ctx, program))
      .child(Text::new(&ctx.t("connections.availableModels")).variant(ExtraTypography::CardTitle.role()))
      .child(models)
      .child(Text::new(&ctx.t("connections.dialog.nextStep")).variant(ExtraTypography::Explanation.role()))
      .into()
  }

  fn name_field(&self, ctx: &Ctx) -> Element {
    let invalid = Name::new(&self.name.get()).err();
    let input = text_input(
      ctx,
      "connection-name",
      self.name.clone(),
      invalid.is_some(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(FIELD_HEIGHT));
    let note = match invalid {
      Some(error) => FieldNote::Error(message_text(ctx, &connection_name_error(&error))),
      None => FieldNote::None,
    };
    field(&ctx.t("connections.dialog.name"), input.into(), note)
  }

  /// The provider select; fixed while editing, since a connection keeps its provider.
  fn provider_field(&self, ctx: &Ctx, provider: AgentProvider) -> Element {
    let label = ctx.t("connections.dialog.provider");
    if matches!(self.context.mode, DialogMode::Edit(_)) {
      let value = ctx.t(program_key(provider));
      let control = value_box(ctx, "connection-provider", &value, ValueTone::Value, None);
      return field(&label, control, FieldNote::None);
    }
    let options = AgentProvider::ALL.map(|provider| (ProviderChoice(provider), ctx.t(program_key(provider))));
    let chevron = ctx.theme().palette_color(PaletteColor::TextSecondary);
    let select = Select::new(self.provider.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FIELD_HEIGHT))
      .style_with(|style| {
        style
          .trigger(select_trigger(ctx, FIELD_HEIGHT))
          .chevron_color(chevron)
          .chevron_size(SELECT_CHEVRON_SIZE)
      });
    let control = Column::new()
      .id("connection-provider")
      .width(Dimension::Pct(100.0))
      .child(select);
    field(&label, control.into(), FieldNote::None)
  }

  /// The program file: found on the `PATH`, or chosen with the file picker.
  fn program_field(&self, ctx: &Ctx, program_name: &str, program: &Program, pick: &PickAction) -> Element {
    let pick = pick.clone();
    let title: Arc<str> = ctx.t_args("connections.dialog.chooseProgram", [("program", program_name)]);
    let choose: Box<dyn Fn() + Send + Sync> = Box::new(move || pick.run(title.clone()));
    let (text, tone) = match &program.executable {
      Some(path) => (display::shorten_path(path, PATH_CHARS), ValueTone::Value),
      None => (
        ctx.t("connections.dialog.noProgram").to_string(),
        ValueTone::Placeholder,
      ),
    };
    let control = value_box(
      ctx,
      "connection-program",
      &text,
      tone,
      Some((Icon::FolderSearch, choose)),
    );
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(field(&ctx.t("connections.property.program"), control, FieldNote::None))
      .child(Text::new(&self.program_hint(ctx, program_name, program.found)).variant(ExtraTypography::Footnote.role()))
      .into()
  }

  fn program_hint(&self, ctx: &Ctx, program_name: &str, found: usize) -> Arc<str> {
    match (&self.picked.get().0, found) {
      (Some(_), _) => ctx.t("connections.dialog.programChosen"),
      (None, 0) => ctx.t_args("connections.dialog.programMissing", [("program", program_name)]),
      (None, 1) => ctx.t("connections.dialog.programFoundOne"),
      (None, count) => ctx.t_args(
        "connections.dialog.programFoundMany",
        [("count", count.to_string().as_str())],
      ),
    }
  }

  /// The check's outcome and Recheck (`VFEkh`).
  fn verification_row(&self, ctx: &Ctx, program: &Program) -> Element {
    let (glyph, status, detail) = match &program.verification {
      Verification::Detecting => (CHECKING.0, CHECKING.1, ctx.t("connections.dialog.detecting")),
      Verification::Checking => (CHECKING.0, CHECKING.1, ctx.t("connections.dialog.checking")),
      Verification::NotFound => (
        Icon::AlertCircle,
        "connections.status.executableMissing",
        ctx.t("connections.dialog.chooseToCheck"),
      ),
      Verification::Failed(error) => (
        Icon::AlertCircle,
        "connections.status.couldNotRun",
        message_text(ctx, error),
      ),
      Verification::Done(check) => {
        let (glyph, status) = status_label(check.result.status());
        (glyph, status, check_detail(ctx, program.provider, &check.result))
      }
    };
    let recheck = ActionButton::new(ctx.t("connections.recheck"))
      .icon(Icon::RefreshCw)
      .id("connection-recheck")
      .height(Dimension::Px(RECHECK_HEIGHT));
    let recheck = match program.verification {
      Verification::Done(_) | Verification::Failed(_) => {
        let attempt = self.attempt.clone();
        recheck.on_click(move || attempt.update(|attempt| *attempt += 1))
      }
      // Nothing to check again yet: the check runs, or there is no program to check.
      Verification::Detecting | Verification::Checking | Verification::NotFound => {
        recheck.on_click(|| {}).enabled(false)
      }
    };
    Row::new()
      .id("connection-verification")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(label(glyph, &ctx.t(status)))
      .child(Text::new(&detail).variant(TypographyStyle::Caption).flex(1.0))
      .child(recheck.render(ctx))
      .into()
  }
}

/// What a finished check found, in one line: the version and whether the CLI is signed in.
fn check_detail(ctx: &Ctx, provider: AgentProvider, result: &CheckResult) -> Arc<str> {
  let program = ctx.t(program_key(provider));
  let CheckResult::Available { version, .. } = result else {
    let key = match result.status() {
      ConnectionStatus::NotRecognized => "connections.dialog.notRecognized",
      ConnectionStatus::ExecutableMissing => "connections.detail.executableMissing",
      _ => "connections.detail.couldNotRun",
    };
    return ctx.t_args(key, [("program", &*program)]);
  };
  let key = match result.status() {
    ConnectionStatus::Connected => "connections.dialog.verifiedSignedIn",
    ConnectionStatus::SignInRequired => "connections.dialog.verifiedSignedOut",
    _ => "connections.dialog.verifiedSignInUnknown",
  };
  ctx.t_args(key, [("program", &*program), ("version", version.as_str())])
}

pub(super) fn connection_name_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.connectionNameRequired"),
    NameError::TooLong { .. } => messages::name_error(error),
  }
}

/// Two fields side by side (`PVMae`).
fn pair(left: Element, right: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Column::new().flex(1.0).child(left))
    .child(Column::new().flex(1.0).child(right))
    .into()
}

/// A field whose value is fixed for agent providers on this computer.
fn fixed(ctx: &Ctx, id: &'static str, label: &str, value: &str) -> Element {
  let control = value_box(ctx, id, &ctx.t(value), ValueTone::Value, None);
  field(&ctx.t(label), control, FieldNote::None)
}
