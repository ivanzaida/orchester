//! The secret dialog's frame and fields (`E213dn`): header with the owning project and a close
//! control (`OVD2C`), name and type, the masked value with its hint, project and environment, a
//! note on how tools use the secret, and Cancel and Save secret (`gfgF0`). Replacing a value keeps
//! every field but the value fixed.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, ScrollVertical, Select, Spacer, Text},
  core::{ElementRef, Signal, signal::SignalValue},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension},
};
use orchester_core::secret::{Environment, SecretKind};

use super::{
  dialog::{DialogPurpose, EnvironmentChoice, KindChoice, NameCheck, SecretDialog},
  labels::{environment_key, kind_key},
  library::handling_notice,
};
use crate::{
  messages::Message,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    backdrop::backdrop,
    button::{ActionButton, SKIPPED_BY_TAB},
    field::{ValueTone, field_error, select_style, text_input, value_box},
    form::{Submit, form},
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop, pressable, surface},
  },
};

/// Dialog, header, action bar, field and close control sizes (`E213dn`, `OVD2C`, `gfgF0`,
/// `f2bgO`, `ct5lH`). The fields scroll when longer notes do not fit the dialog's height.
const DIALOG_WIDTH: f32 = 752.0;
const DIALOG_HEIGHT: f32 = 636.0;
const HEADER_HEIGHT: f32 = 78.0;
const ACTIONS_HEIGHT: f32 = 74.0;
const FIELD_HEIGHT: f32 = 40.0;
const BUTTON_HEIGHT: f32 = 36.0;
const CLOSE_SIZE: f32 = 32.0;

/// Whether Save can be pressed, whether it runs, and why it last failed.
pub(super) type SaveState = (Submit, bool, Option<Message>);

impl SecretDialog {
  pub(super) fn frame(&self, ctx: &Ctx, frame: &ElementRef, check: &NameCheck, save: SaveState) -> Element {
    let (submit, saving, error) = save;
    let dialog = Column::new()
      .id("secret-dialog")
      .ref_element(frame.clone())
      .focusable(true)
      .tab_index(SKIPPED_BY_TAB)
      .width(Dimension::Px(DIALOG_WIDTH))
      .height(Dimension::Px(DIALOG_HEIGHT))
      .background(ExtraColor::Surface.role())
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .child(self.header(ctx))
      .child(
        ScrollVertical::new(self.fields(ctx, check))
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      )
      .child(self.actions(ctx, submit.clone(), saving, error));
    backdrop(form(submit, dialog))
  }

  fn header(&self, ctx: &Ctx) -> Element {
    let project = self.context.project.project.name().as_str();
    let (title, subtitle) = match &self.context.purpose {
      DialogPurpose::Add => (
        ctx.t("secrets.dialog.addTitle"),
        ctx.t_args("secrets.dialog.addSubtitle", [("project", project)]),
      ),
      DialogPurpose::Replace(secret) => (
        ctx.t("secrets.dialog.replaceTitle"),
        ctx.t_args(
          "secrets.dialog.replaceSubtitle",
          [
            ("name", secret.name().as_str()),
            ("project", project),
            ("environment", &*ctx.t(environment_key(secret.environment()))),
          ],
        ),
      ),
    };
    let close = self.close();
    let close_button = keyboard_stop(Button::empty().button(), Ring::Outer)
      .id("secret-dialog-close")
      .size(Dimension::Px(CLOSE_SIZE), Dimension::Px(CLOSE_SIZE))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Md)
      .cursor(CursorIcon::Pointer)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .on_click(move |_| close())
      .child(icon(Icon::X, IconSize::Notice, PaletteColor::TextSecondary));
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
          .child(Text::new(&title).variant(ExtraTypography::DialogTitle.role()))
          .child(Text::new(&subtitle).variant(TypographyStyle::Description)),
      )
      .child(close_button)
      .into()
  }

  fn fields(&self, ctx: &Ctx, check: &NameCheck) -> Element {
    let project = self.context.project.project.name().as_str();
    let project_field = labelled(
      &ctx.t("secrets.dialog.project"),
      value_box(ctx, "secret-project", project, ValueTone::Value, None),
      None,
    );
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Form.role())
      .child(pair(self.name_field(ctx, check), self.kind_field(ctx)))
      .child(self.value_field(ctx, check))
      .child(pair(project_field, self.environment_field(ctx)))
      .child(self.notice(ctx, check))
      .into()
  }

  fn name_field(&self, ctx: &Ctx, check: &NameCheck) -> Element {
    let label = ctx.t("secrets.dialog.name");
    if let DialogPurpose::Replace(secret) = &self.context.purpose {
      let control = value_box(ctx, "secret-name", secret.name().as_str(), ValueTone::Value, None);
      return labelled(&label, control, None);
    }
    let project = self.context.project.project.name().as_str();
    let note = match check {
      NameCheck::Invalid(message) => Some(message_text(ctx, message)),
      NameCheck::Taken(taken) => Some(ctx.t_args(
        "secrets.dialog.nameTaken",
        [
          ("project", project),
          ("environment", &*ctx.t(environment_key(taken.environment()))),
        ],
      )),
      NameCheck::Empty | NameCheck::Free(_) => None,
    };
    let input = text_input(
      ctx,
      "secret-name",
      self.name.clone(),
      note.is_some(),
      TypographyStyle::FieldLabel,
    )
    .height(Dimension::Px(FIELD_HEIGHT));
    labelled(&label, input.into(), note.map(|note| field_note(&note)))
  }

  fn kind_field(&self, ctx: &Ctx) -> Element {
    let label = ctx.t("secrets.dialog.type");
    if let DialogPurpose::Replace(secret) = &self.context.purpose {
      let value = ctx.t(kind_key(secret.kind()));
      return labelled(
        &label,
        value_box(ctx, "secret-kind", &value, ValueTone::Value, None),
        None,
      );
    }
    let options = SecretKind::ALL.map(|kind| (KindChoice(kind), ctx.t(kind_key(kind))));
    labelled(&label, select(ctx, "secret-kind", self.kind.clone(), options), None)
  }

  fn environment_field(&self, ctx: &Ctx) -> Element {
    let label = ctx.t("secrets.dialog.environment");
    if let DialogPurpose::Replace(secret) = &self.context.purpose {
      let value = ctx.t(environment_key(secret.environment()));
      let control = value_box(ctx, "secret-environment", &value, ValueTone::Value, None);
      return labelled(&label, control, None);
    }
    let options =
      Environment::ALL.map(|environment| (EnvironmentChoice(environment), ctx.t(environment_key(environment))));
    labelled(
      &label,
      select(ctx, "secret-environment", self.environment.clone(), options),
      None,
    )
  }

  /// The value, masked on screen and in the MCP tree, with how it is kept (`p10Ig`).
  fn value_field(&self, ctx: &Ctx, check: &NameCheck) -> Element {
    let input = text_input(
      ctx,
      "secret-value",
      self.value.clone(),
      false,
      TypographyStyle::FieldLabel,
    )
    .mask()
    .height(Dimension::Px(FIELD_HEIGHT));
    let hint = match (&self.context.purpose, check) {
      (DialogPurpose::Replace(_), _) => "secrets.dialog.valueHintReplace",
      (DialogPurpose::Add, NameCheck::Taken(_)) => "secrets.dialog.valueHintTaken",
      (DialogPurpose::Add, _) => "secrets.dialog.valueHint",
    };
    let hint = Text::new(&ctx.t(hint))
      .variant(ExtraTypography::Hint.role())
      .color(PaletteColor::TextMuted);
    labelled(&ctx.t("secrets.dialog.value"), input.into(), Some(hint.into()))
  }

  /// How tools use the secret (`SxppF`); for a name that is taken, the existing secret and how to
  /// open it (`ViaaU`).
  fn notice(&self, ctx: &Ctx, check: &NameCheck) -> Element {
    match (&self.context.purpose, check) {
      (DialogPurpose::Replace(_), _) => handling_notice(&ctx.t("secrets.dialog.replaceNotice"), None),
      (DialogPurpose::Add, NameCheck::Taken(taken)) => {
        let (show, id) = (self.context.show.clone(), taken.id());
        let open = pressable(surface().id("secret-open-existing"), move || show(id))
          .align_items(Alignment::Center)
          .spacing(SpacingSize::Xs)
          .child(Text::new(&ctx.t("secrets.dialog.openExisting")).variant(TypographyStyle::Link))
          .child(icon(Icon::ArrowRight, IconSize::Small, PaletteColor::TextPrimary));
        let text = ctx.t_args("secrets.dialog.takenNotice", [("name", taken.name().as_str())]);
        handling_notice(&text, Some(open.into()))
      }
      (DialogPurpose::Add, _) => handling_notice(&ctx.t("secrets.dialog.addNotice"), None),
    }
  }

  fn actions(&self, ctx: &Ctx, submit: Submit, saving: bool, error: Option<Message>) -> Element {
    let (label, glyph) = match (&self.context.purpose, saving) {
      (DialogPurpose::Add, false) => ("secrets.dialog.save", Icon::LockKeyhole),
      (DialogPurpose::Add, true) => ("secrets.dialog.saving", Icon::LockKeyhole),
      (DialogPurpose::Replace(_), false) => ("secrets.replaceValue", Icon::RefreshCw),
      (DialogPurpose::Replace(_), true) => ("secrets.dialog.replacing", Icon::RefreshCw),
    };
    let save = ActionButton::new(ctx.t(label))
      .primary()
      .icon(glyph)
      .id("secret-save")
      .in_form()
      .height(Dimension::Px(BUTTON_HEIGHT));
    let save = match submit {
      Some(submit) => save.on_click(move || submit()),
      // On once a name and a value are entered, and while no save runs.
      None => save.on_click(|| {}).enabled(false),
    };
    let close = self.close();
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ACTIONS_HEIGHT))
      .padding_horizontal(ExtraSpacing::Rail.role())
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .with_children(error.map(|error| {
        Column::new()
          .id("secret-save-error")
          .child(field_error(&message_text(ctx, &error)))
      }))
      .child(Spacer::new().flex(1.0))
      .child(
        ActionButton::new(ctx.t("common.cancel"))
          .id("secret-cancel")
          .in_form()
          .height(Dimension::Px(BUTTON_HEIGHT))
          .on_click(close)
          .render(ctx),
      )
      .child(save.render(ctx))
      .into()
  }
}

/// A label over its control, and a note under it (`hsrR6`).
fn labelled(label: &str, control: Element, note: Option<Element>) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Field.role())
    .child(Text::new(label).variant(TypographyStyle::Caption))
    .child(control)
    .with_children(note)
    .into()
}

/// Why the name cannot be saved (`zfPkd`): 10px text under the field.
fn field_note(text: &Arc<str>) -> Element {
  Text::new(text)
    .variant(ExtraTypography::Footnote.role())
    .color(PaletteColor::TextPrimary)
    .id("secret-name-note")
    .into()
}

/// A select drawn like a field (`ZGwlv`).
fn select<T, const N: usize>(ctx: &Ctx, id: &'static str, value: Signal<T>, options: [(T, Arc<str>); N]) -> Element
where
  T: Clone + PartialEq + Send + Sync + 'static + SignalValue,
  Vec<T>: SignalValue,
{
  let select = Select::new(value)
    .options(options)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(FIELD_HEIGHT))
    .style(select_style(ctx, FIELD_HEIGHT));
  Column::new().id(id).width(Dimension::Pct(100.0)).child(select).into()
}

/// Two fields side by side (`g1jO6Q`).
fn pair(left: Element, right: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Step.role())
    .child(Column::new().flex(1.0).child(left))
    .child(Column::new().flex(1.0).child(right))
    .into()
}
