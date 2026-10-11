//! Fields and footer of the inline credential editor, including name-check and write-failure states.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::secret::{Environment, SecretKind, SecretName, SecretNameError};
use orchester_workspace::secrets::{data::ReplacementPermission, form::NameCheck};

use super::{Props, SecretForm};
use crate::{
  screens::secrets::{
    inspector,
    state::{EnvironmentChoice, KindChoice},
    words, work,
  },
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, SecretsSpacing, SecretsText},
  ui::{
    avatar::{credential_avatar, credential_project_avatar},
    button::{ActionButton, ButtonKind},
    field::{FieldInput, SelectField, field, field_error},
    icon::Icon,
    icon_button::IconButton,
    message::message_text,
    notice::{handling_notice, handling_notice_action},
    unavailable::reason_line,
  },
};

pub(super) fn new_header(
  ctx: &mut Ctx,
  props: &Props,
  cancel: std::sync::Arc<dyn Fn() + Send + Sync>,
  pending: bool,
) -> Element {
  let mut close = IconButton::new("secret-form-close", Icon::X, ctx.t("secrets.cancel"))
    .inspector()
    .in_form();
  if !pending {
    close = close.on_click(move || cancel());
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Lg)
        .right(ExtraSpacing::Loose.role())
        .top(SecretsSpacing::Body.role())
        .bottom(SecretsSpacing::Body.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(credential_avatar("secret-form-tile", Icon::KeyRound, true))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t("secrets.form.title")).variant(SecretsText::InspectorTitle.role()))
        .child(
          Text::new(&ctx.t_args(
            "secrets.form.subtitle",
            [("project", props.project.open.project.name().as_str())],
          ))
          .variant(TypographyStyle::Description),
        ),
    )
    .child(close.render(ctx))
    .into()
}
fn project_row(ctx: &Ctx, props: &Props) -> Row {
  let name = props.project.open.project.name().as_str();
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(credential_project_avatar("secret-project-avatar", name))
    .child(Text::new(name).variant(TypographyStyle::Label))
    .child(
      Text::new(&ctx.t("secrets.form.projectNote"))
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
    )
}

impl SecretForm {
  pub(super) fn fields(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, pending: bool) -> Element {
    let mut body = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, SecretsSpacing::Body.role()))
      .spacing(SecretsSpacing::Body.role());
    if let Some(secret) = &props.secret {
      body = body
        .child(Text::new(&ctx.t("secrets.rotation.title")).variant(SecretsText::InspectorTitle.role()))
        .child(
          Text::new(&ctx.t_args(
            "secrets.rotation.subtitle",
            [
              ("name", secret.name().as_str()),
              ("project", props.project.open.project.name().as_str()),
              ("environment", &ctx.t(words::environment_key(secret.environment()))),
            ],
          ))
          .variant(ExtraTypography::Note.role()),
        );
    } else {
      body = body
        .child(self.name_field(ctx, props, check, pending))
        .child(self.kind_field(ctx, pending));
    }
    let mut value = FieldInput::new("secret-value", ctx.t("secrets.form.value"), self.value.clone()).masked();
    if pending {
      value = value.locked();
    }
    let replacing = props.secret.is_some();
    let label = ctx.t(if replacing {
      "secrets.rotation.value"
    } else {
      "secrets.form.value"
    });
    let hint = ctx.t(if replacing {
      "secrets.rotation.hint"
    } else {
      "secrets.form.valueHint"
    });
    body = body.child(field(
      "secret-value-field",
      Some(&label),
      value.render(ctx),
      Some(&hint),
    ));
    if !replacing {
      body = body
        .child(field(
          "secret-project-field",
          Some(&ctx.t("secrets.project")),
          project_row(ctx, props),
          None,
        ))
        .child(self.environment_field(ctx, pending));
    }
    body = body.child(self.notice(ctx, props, check));
    if let Some(secret) = &props.secret {
      body = body.child(inspector::uses(ctx, &props.project, &props.listed, secret));
    }
    body.into()
  }

  fn name_field(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, pending: bool) -> Element {
    let hint = match check {
      NameCheck::Invalid(SecretNameError::TooLong { .. }) => {
        Some(ctx.t_args("secrets.form.tooLong", [("max", SecretName::MAX_CHARS.to_string())]))
      }
      NameCheck::Invalid(_) => Some(ctx.t("secrets.form.invalid")),
      NameCheck::Taken(secret) => Some(ctx.t_args(
        "secrets.form.taken",
        [
          ("project", props.project.open.project.name().as_str()),
          ("environment", &ctx.t(words::environment_key(secret.environment()))),
        ],
      )),
      _ => None,
    };
    let mut input = FieldInput::new("secret-name", ctx.t("secrets.form.name"), self.name.clone()).mono();
    if pending {
      input = input.locked();
    }
    if hint.is_some() {
      input = input.invalid();
    }
    field(
      "secret-name-field",
      Some(&ctx.t("secrets.form.name")),
      input.render(ctx),
      None,
    )
    .with_children(hint.map(|hint| {
      Text::new(&hint)
        .id("secret-name-field-hint")
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::Danger)
    }))
    .into()
  }

  fn kind_field(&self, ctx: &mut Ctx, pending: bool) -> Element {
    let label = ctx.t("secrets.form.kind");
    let options = SecretKind::ALL
      .into_iter()
      .map(|kind| SelectOption::new(KindChoice(kind), ctx.t(words::kind_key(kind))))
      .collect();
    let control = if pending {
      FieldInput::new(
        "secret-kind",
        label.clone(),
        lurq::core::Signal::new(ctx.t(words::kind_key(self.kind.get().0)).to_string()),
      )
      .locked()
      .render(ctx)
    } else {
      SelectField::new("secret-kind", label.clone(), self.kind.clone(), options).render(ctx)
    };
    field("secret-kind-field", Some(&label), control, None).into()
  }

  fn environment_field(&self, ctx: &mut Ctx, pending: bool) -> Element {
    let label = ctx.t("secrets.environment");
    let options = Environment::ALL
      .into_iter()
      .map(|environment| {
        SelectOption::new(
          EnvironmentChoice(Some(environment)),
          ctx.t(words::environment_key(environment)),
        )
      })
      .collect();
    let control = if pending {
      FieldInput::new(
        "secret-form-environment",
        label.clone(),
        lurq::core::Signal::new(
          ctx
            .t(words::environment_key(
              self.environment.get().0.expect("an environment is chosen"),
            ))
            .to_string(),
        ),
      )
      .locked()
      .render(ctx)
    } else {
      SelectField::new(
        "secret-form-environment",
        label.clone(),
        self.environment.clone(),
        options,
      )
      .render(ctx)
    };
    field("secret-form-environment-field", Some(&label), control, None).into()
  }

  fn notice(&self, ctx: &Ctx, props: &Props, check: &NameCheck) -> Element {
    let text = match (&props.secret, check) {
      (Some(_), _) => ctx.t("secrets.rotation.notice"),
      (None, NameCheck::Taken(secret)) => ctx.t_args("secrets.form.takenNotice", [("name", secret.name().as_str())]),
      _ => ctx.t("secrets.form.notice"),
    };
    if props.secret.is_none()
      && let NameCheck::Taken(secret) = check
    {
      let (cancel, view, id) = (self.cancel(props), props.view.clone(), secret.id());
      let environment = secret.environment();
      return handling_notice_action(
        "secret-form-notice",
        "secret-open-existing",
        Icon::ShieldCheck,
        &text,
        &ctx.t("secrets.form.open"),
        move || {
          cancel();
          view.reveal(id, environment);
        },
      )
      .into();
    }
    handling_notice("secret-form-notice", Icon::ShieldCheck, &text).into()
  }

  pub(super) fn footer(&self, ctx: &mut Ctx, props: &Props, check: &NameCheck, action: &work::Action) -> Element {
    let state = action.state().get();
    let submit = self.save(ctx, props, check, action, ReplacementPermission::UnusedOnly);
    if self.asking.get()
      && let Some(save) = self.save(ctx, props, check, action, ReplacementPermission::PersonConfirmed)
    {
      return self.question(ctx, save);
    }
    let pending = state.is_pending();
    let replacing = props.secret.is_some();
    let label = match (replacing, pending) {
      (true, true) => "secrets.rotation.replacing",
      (true, false) => "secrets.replace",
      (false, true) => "secrets.form.saving",
      (false, false) => "secrets.form.save",
    };
    let mut save = ActionButton::new("secret-save", ButtonKind::Primary, ctx.t(label))
      .icon(if replacing { Icon::RefreshCw } else { Icon::LockKeyhole })
      .in_form();
    if let Some(submit) = &submit {
      let (asking, submit) = (self.asking.clone(), submit.clone());
      let needs_person = props
        .secret
        .as_ref()
        .is_some_and(|secret| props.listed.replacement_needs_person(secret.id()));
      save = save.on_click(move || {
        if needs_person {
          asking.set(true);
        } else {
          submit();
        }
      });
    }
    if pending {
      save = save.loading();
    }
    let cancel = self.cancel(props);
    let cancel = ActionButton::new("secret-cancel", ButtonKind::Secondary, ctx.t("secrets.cancel"))
      .in_form()
      .on_click(move || cancel());
    let mut footer = Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Snug.role())
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border));
    if let Some(error) = &state.error {
      footer = footer.child(field_error("secret-save-error", &message_text(ctx, error)));
    }
    footer = footer.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(Spacer::new().flex(1.0))
        .child(if pending { cancel.disabled() } else { cancel })
        .child(save),
    );
    if submit.is_none() && !pending {
      footer = footer.child(reason_line("secret-save-reason", &self.reason(ctx, check, replacing)));
    }
    footer.into()
  }

  fn reason(&self, ctx: &Ctx, check: &NameCheck, replacing: bool) -> std::sync::Arc<str> {
    if replacing {
      return ctx.t("secrets.rotation.missing");
    }
    match check {
      NameCheck::Invalid(SecretNameError::TooLong { .. }) => ctx.t("secrets.form.longReason"),
      NameCheck::Invalid(_) => ctx.t("secrets.form.invalidReason"),
      NameCheck::Taken(secret) => ctx.t_args(
        "secrets.form.takenReason",
        [("environment", ctx.t(words::environment_key(secret.environment())))],
      ),
      _ => ctx.t("secrets.form.missing"),
    }
  }
}
