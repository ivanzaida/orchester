//! *API key · project secret* of the native connection dialog (04.H1 `Nyjtn` in `Byo8K`, 04.H2 `FOKH4` in
//! `rkeoO`): a select of the project's secrets with *New secret…* beside it and what the connection
//! keeps; or, for a secret created with the connection, *Choose an existing secret* and a bordered panel
//! (`lWlxW`) with its name and masked value side by side, its environment (chosen and defaulted as in
//! Secrets) and where it is saved.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Select, SelectOption, Spacer, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{connection::AgentProvider, secret::Environment};
use zeroize::Zeroize;

use super::{
  form::pair,
  native_dialog::{KeyChoice, KeyEnvironment, NativeDialog},
  native_form::{FIELD_HEIGHT, note, select},
};
use crate::{
  messages::Message,
  screens::project::secrets::labels::environment_key,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{FieldNote, field, select_style, text_input},
    icon::Icon,
    pressable::{pressable, surface},
  },
};

impl NativeDialog {
  pub(super) fn key_field(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    if self.key.get() == KeyChoice::New {
      return self.new_key(ctx, error);
    }
    let mut options = vec![SelectOption::new(KeyChoice::None, ctx.t("connections.native.key.none"))];
    options.extend(self.context.secrets.iter().map(|secret| {
      let label = ctx.t_args(
        "connections.native.key.option",
        [
          ("name", secret.name().as_str()),
          ("environment", &*ctx.t(environment_key(secret.environment()))),
        ],
      );
      SelectOption::new(KeyChoice::Existing(secret.id()), label)
    }));
    let key_select = Select::new(self.key.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FIELD_HEIGHT))
      .style(select_style(ctx, FIELD_HEIGHT));
    let key = self.key.clone();
    let new = ActionButton::new(ctx.t("connections.native.key.new"))
      .icon(Icon::KeyRound)
      .id("native-key-new")
      .in_form()
      .height(Dimension::Px(FIELD_HEIGHT))
      .on_click(move || key.set(KeyChoice::New))
      .render(ctx);
    // The select takes its own id, for tools that choose an option; the column holds what it shows.
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .overflow_visible()
      .child(
        Column::new()
          .id("native-key")
          .flex(1.0)
          .child(Element::from(key_select).id("native-key-select")),
      )
      .child(new);
    let shown = error.filter(|_| self.key.get() != KeyChoice::None);
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(row)
      .child(Text::new(&ctx.t("connections.native.key.keepsHint")).variant(ExtraTypography::Hint.role()));
    field(&ctx.t("connections.native.key.label"), content.into(), note(ctx, shown))
  }

  /// A secret created with the connection (`FOKH4`).
  fn new_key(&self, ctx: &Ctx, error: Option<&Message>) -> Element {
    let named = !self.key_name.get().trim().is_empty();
    let name = text_input(
      ctx,
      "native-key-name",
      self.key_name.clone(),
      named && error.is_some(),
      ExtraTypography::MonoField.role(),
    )
    .height(Dimension::Px(FIELD_HEIGHT))
    .placeholder(&ctx.t(match self.provider {
      AgentProvider::OpenAiApi => "connections.native.key.namePlaceholder.openai",
      _ => "connections.native.key.namePlaceholder.anthropic",
    }));
    let value = text_input(
      ctx,
      "native-key-value",
      self.key_value.clone(),
      false,
      ExtraTypography::MonoField.role(),
    )
    .mask()
    .height(Dimension::Px(FIELD_HEIGHT));
    let environment = self.key_environment.get().0;
    let environments = Environment::ALL
      .into_iter()
      .map(|environment| SelectOption::new(KeyEnvironment(environment), ctx.t(environment_key(environment))))
      .collect();
    let hint = ctx.t_args(
      "connections.native.key.newHint",
      [
        ("project", self.context.project.project.name().as_str()),
        ("environment", &*ctx.t(environment_key(environment))),
      ],
    );
    let panel = Column::new()
      .id("native-key-new-panel")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Group.role())
      .background(PaletteColor::SurfacePanel)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(ExtraRadius::Notice.role())
      .overflow_visible()
      .child(pair(
        field(
          &ctx.t("connections.native.key.name"),
          name.into(),
          note(ctx, error.filter(|_| named)),
        ),
        field(&ctx.t("connections.native.key.value"), value.into(), FieldNote::None),
      ))
      // Half width, under the name, as Secrets pairs its environment (`E213dn`).
      .child(pair(
        select(
          ctx,
          "native-key-environment",
          "connections.native.key.environment",
          self.key_environment.clone(),
          environments,
        ),
        Column::new().into(),
      ))
      .child(Text::new(&hint).variant(ExtraTypography::Hint.role()));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(self.new_key_label(ctx))
      .child(panel)
      .into()
  }

  /// *API key · new project secret* and *Choose an existing secret* (`P1whyS`).
  fn new_key_label(&self, ctx: &Ctx) -> Element {
    let (key, typed) = (self.key.clone(), self.key_value.clone());
    // A value typed for the new secret is wiped when an existing secret is chosen instead.
    let existing = pressable(surface().id("native-key-existing"), move || {
      typed.update(|text| text.zeroize());
      key.set(KeyChoice::None);
    })
    .child(
      Text::new(&ctx.t("connections.native.key.chooseExisting"))
        .variant(TypographyStyle::Caption)
        .color(PaletteColor::TextPrimary),
    );
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(Text::new(&ctx.t("connections.native.key.newLabel")).variant(TypographyStyle::Caption))
      .child(Spacer::new().flex(1.0))
      .child(existing)
      .into()
  }
}
