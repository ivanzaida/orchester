//! The secrets of the chosen environment (`Cxn5H`) beside the shown one's details (`CQxCb`), or
//! what secrets are for when the project has none.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Select, Spacer, Text},
  layout::Alignment,
  node::{CursorIcon, Element, dimension::Dimension},
};
use orchester_core::{
  id::SecretId,
  secret::{Environment, Secret},
};

use super::{
  DialogMode, EnvironmentFilter, SecretsScreen, centered,
  inspector::{InspectorActions, inspector},
  labels::environment_key,
  row::{SecretRow, secret_row},
};
use crate::{
  navigation::route::Page,
  secrets::data::ProjectSecrets,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    field::select_style,
    gutter::{beside_gutter, clear_of_gutter},
    icon::{Icon, IconSize, icon},
    panel::RecoveryPanel,
  },
  workspace::OpenProject,
};

/// Environment filter size (`p3oCT`).
const FILTER_WIDTH: f32 = 172.0;
const FILTER_HEIGHT: f32 = 34.0;

impl SecretsScreen {
  pub(super) fn library(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    data: &ProjectSecrets,
    selected: Option<SecretId>,
  ) -> Element {
    if data.secrets.is_empty() {
      return self.empty(ctx);
    }
    let filter = self.filter.get().0;
    let listed: Vec<&Secret> = data
      .secrets
      .iter()
      .filter(|secret| filter.is_none_or(|environment| secret.environment() == environment))
      .collect();
    let shown = listed
      .iter()
      .copied()
      .find(|secret| Some(secret.id()) == selected)
      .or_else(|| listed.first().copied());
    let project = open.project.name().as_str();
    let rows: Vec<Element> = listed
      .iter()
      .map(|secret| {
        let id = secret.id();
        let row = SecretRow {
          secret,
          project,
          uses: data.use_counts(id),
          selected: shown.is_some_and(|shown| shown.id() == id),
        };
        secret_row(ctx, &row, self.nav.link(Page::Secrets(Some(id))))
      })
      .collect();
    let list = self.list_column(ctx, filter, rows, listed.len());
    let Some(shown) = shown else {
      return list;
    };
    let (replace, archive) = (self.dialog.clone(), self.dialog.clone());
    let (replacing, archiving) = (shown.clone(), shown.clone());
    let actions = InspectorActions {
      replace: Arc::new(move || replace.set(DialogMode::Replace(replacing.clone()))),
      archive: Arc::new(move || archive.set(DialogMode::Archive(archiving.clone()))),
    };
    let uses: Vec<_> = data.used_by(shown.id()).collect();
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list)
      .child(inspector(
        ctx,
        (shown, &self.details_scroll),
        project,
        (&uses, data.use_counts(shown.id())),
        &self.nav,
        &actions,
      ))
      .into()
  }

  /// The project has no secrets: what they are for, and adding one.
  fn empty(&self, ctx: &Ctx) -> Element {
    centered(
      RecoveryPanel::new(
        Icon::KeyRound,
        &ctx.t("secrets.emptyTitle"),
        &ctx.t("secrets.emptyDetail"),
      )
      .action(self.add_button(ctx, "empty-add-secret"))
      .into(),
    )
  }

  /// Filter and count, the rows, how values are handled, recent access and who owns bindings
  /// (`Cxn5H`).
  fn list_column(&self, ctx: &Ctx, filter: Option<Environment>, rows: Vec<Element>, count: usize) -> Element {
    let rows = if rows.is_empty() {
      let environment = filter.map_or_else(|| ctx.t("secrets.filter.all"), |env| ctx.t(environment_key(env)));
      vec![
        Text::new(&ctx.t_args("secrets.emptyEnvironment", [("environment", &*environment)]))
          .variant(TypographyStyle::Caption)
          .into(),
      ]
    } else {
      rows
    };
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(
        Column::new()
          .id("secret-list")
          .width(Dimension::Pct(100.0))
          .with_children(rows),
      )
      .child(handling_notice(&ctx.t("secrets.handling"), None))
      .child(recent_access(ctx));
    // The secrets scroll beside the gutter; the filter and the ownership line keep out of it.
    Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Rail.role())
      .padding_right(beside_gutter(ctx, ExtraSpacing::Rail.role()))
      .spacing(SpacingSize::Lg)
      .child(clear_of_gutter(self.filters(ctx, count)))
      .child(
        ScrollVertical::new(content)
          .id("secret-list-scroll")
          .with_scroll_state(self.list_scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .scrollbar(gutter_scrollbar()),
      )
      .child(clear_of_gutter(
        Text::new(&ctx.t("secrets.ownership")).variant(TypographyStyle::Caption),
      ))
      .into()
  }

  /// The environment filter and how many secrets it lists (`pKTF5`).
  fn filters(&self, ctx: &Ctx, count: usize) -> Element {
    let options = [
      None,
      Some(Environment::Development),
      Some(Environment::Staging),
      Some(Environment::Production),
    ]
    .map(|environment| {
      let label = environment.map_or_else(|| ctx.t("secrets.filter.all"), |env| ctx.t(environment_key(env)));
      (EnvironmentFilter(environment), label)
    });
    let select = Select::new(self.filter.clone())
      .options(options)
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(FILTER_HEIGHT))
      .style(select_style(ctx, FILTER_HEIGHT));
    let key = if count == 1 {
      "secrets.count.one"
    } else {
      "secrets.count.other"
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .child(
        Column::new()
          .id("secret-environment-filter")
          .width(Dimension::Px(FILTER_WIDTH))
          .child(select),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&ctx.t_args(key, [("count", count.to_string().as_str())]))
          .variant(TypographyStyle::Caption)
          .id("secret-count"),
      )
      .into()
  }
}

/// A shield beside how secret values are handled (`TV2Is`, `SxppF`), with an optional action on
/// the right.
pub(super) fn handling_notice(text: &str, action: Option<Element>) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Stack.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .rounded(ExtraRadius::Notice.role())
    .child(icon(Icon::ShieldCheck, IconSize::Notice, PaletteColor::TextSecondary))
    .child(Text::new(text).variant(ExtraTypography::Explanation.role()).flex(1.0))
    .with_children(action)
    .into()
}

/// Recent access (`o10oq`). Access is recorded once tools can use secrets, which they cannot in
/// this version, so the section says so instead of listing entries.
fn recent_access(ctx: &Ctx) -> Element {
  let audit = Row::new()
    .id("secrets-audit-log")
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .describe("state", "unavailable")
    .cursor(CursorIcon::NotAllowed)
    .child(Text::new(&ctx.t("secrets.auditLog")).variant(TypographyStyle::Caption))
    .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextMuted));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Divider.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .child(Text::new(&ctx.t("secrets.recentAccess")).variant(ExtraTypography::SectionHeading.role()))
        .child(Spacer::new().flex(1.0))
        .child(audit),
    )
    .child(
      Text::new(&ctx.t("secrets.recentAccessUnavailable"))
        .variant(TypographyStyle::Caption)
        .id("secrets-recent-access"),
    )
    .into()
}
