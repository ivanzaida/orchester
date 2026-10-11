//! Environment filter and windowed metadata table (DrLE6).
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, SelectOption, Spacer, Text, VirtualizedList},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::SecretId,
  secret::{Environment, Secret},
};
use orchester_workspace::secrets::data::ProjectSecrets;

use super::{
  state::{EnvironmentChoice, Panel, View},
  words,
};
use crate::{
  theme::{ExtraSpacing, IconSize, SecretsSpacing, SecretsText},
  ui::{
    avatar::credential_avatar,
    button::{ActionButton, ButtonKind},
    field::SelectField,
    icon::{Icon, icon},
    notice::handling_notice,
    state::EmptyState,
    table::{ColumnWidth, TableRow},
    tooltip::Truncated,
  },
};

/// Columns of Byb1r/I2uJV0, a row's 48 height and the filter's 180 width.
const KIND: ColumnWidth = ColumnWidth::Fixed(150.0);
const ENVIRONMENT: ColumnWidth = ColumnWidth::Fixed(110.0);
const USED: ColumnWidth = ColumnWidth::Fixed(120.0);
const ROW_HEIGHT: f32 = 48.0;
const HEADER_HEIGHT: f32 = 32.0;
const FILTER_WIDTH: f32 = 180.0;
/// Compact environment-empty row RlvMO in Ds3OR.
const EMPTY_ROW_HEIGHT: f32 = 76.0;

pub(super) fn column(ctx: &mut Ctx, listed: &ProjectSecrets, view: &View, selected: Option<SecretId>) -> Column {
  let entries: Vec<Secret> = listed.in_environment(view.environment.get().0).cloned().collect();
  let count = entries.len();
  let mut column = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .spacing(SecretsSpacing::Body.role());
  if listed.in_environment(None).next().is_none() {
    column = column.child(project_empty(ctx, view));
  } else if count == 0 {
    column = column
      .child(filters(ctx, view, Some(count)))
      .child(environment_empty(ctx, view));
  } else {
    column = column.child(filters(ctx, view, Some(count)));
    let rows: Arc<Vec<RowProps>> = Arc::new(
      entries
        .into_iter()
        .map(|secret| RowProps {
          used: words::uses(ctx, listed.use_counts(secret.id())),
          unused: listed.use_counts(secret.id()).total() == 0,
          secret,
          selected,
          view: view.clone(),
        })
        .collect(),
    );
    let rows = VirtualizedList::new_shared(ctx, rows)
      .list_key(format!("secrets-{:?}", view.environment.get().0))
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .max_height(Dimension::Px(ROW_HEIGHT * count as f32))
      .mount_keyed::<SecretRow, _, _, _>(|row| row.secret.id().to_string(), Clone::clone);
    column = column.child(
      Column::new()
        .id("secrets-list")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .max_height(Dimension::Px(HEADER_HEIGHT + ROW_HEIGHT * count as f32))
        .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .rounded(RadiusSize::Lg)
        .clip()
        .child(header(ctx))
        .child(rows),
    );
  }
  column
    .child(handling_notice(
      "secrets-handling",
      Icon::ShieldCheck,
      &ctx.t("secrets.handling"),
    ))
    .child(Spacer::new().flex(1.0))
}

fn project_empty(ctx: &Ctx, view: &View) -> Element {
  let panel = view.panel.clone();
  EmptyState::new(
    "secrets-empty",
    Icon::KeyRound,
    ctx.t("secrets.emptyTitle"),
    ctx.t("secrets.emptyDetail"),
  )
  .secrets()
  .action(
    ActionButton::new("secrets-empty-add", ButtonKind::Primary, ctx.t("secrets.add"))
      .icon(Icon::Plus)
      .on_click(move || panel.set(Panel::Add)),
  )
  .action(
    ActionButton::new("secrets-empty-import", ButtonKind::Secondary, ctx.t("secrets.import"))
      .icon(Icon::Download)
      .unavailable(ctx.t("secrets.importUnavailable")),
  )
  .into()
}

fn environment_empty(ctx: &Ctx, view: &View) -> Element {
  let environment = view
    .environment
    .get()
    .0
    .expect("a nonempty project has no rows only under an environment filter");
  Column::new()
    .id("secrets-list")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(header(ctx))
    .child(
      Column::new()
        .id("secrets-empty")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Px(EMPTY_ROW_HEIGHT))
        .padding(Padding::symmetric(
          SecretsSpacing::Cell.role(),
          ExtraSpacing::Tabs.role(),
        ))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(&ctx.t_args(
            "secrets.environmentEmpty",
            [("environment", ctx.t(words::environment_key(environment)))],
          ))
          .variant(SecretsText::EnvironmentEmpty.role()),
        )
        .child(
          Text::new(&ctx.t("secrets.environmentEmptyDetail"))
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted),
        ),
    )
    .into()
}

pub(super) fn filters(ctx: &Ctx, view: &View, count: Option<usize>) -> Row {
  let mut options = vec![SelectOption::new(
    EnvironmentChoice(None),
    ctx.t("secrets.allEnvironments"),
  )];
  options.extend(Environment::ALL.into_iter().map(|environment| {
    SelectOption::new(
      EnvironmentChoice(Some(environment)),
      ctx.t(words::environment_key(environment)),
    )
  }));
  Row::new()
    .id("secrets-filter")
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(
      SelectField::new(
        "secrets-environment",
        ctx.t("secrets.environment"),
        view.environment.clone(),
        options,
      )
      .width(Dimension::Px(FILTER_WIDTH))
      .render(ctx),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(count.map(|count| {
      Text::new(&ctx.t_args(
        if count == 1 {
          "secrets.countOne"
        } else {
          "secrets.count"
        },
        [("count", count.to_string())],
      ))
      .id("secrets-count")
      .variant(TypographyStyle::Caption)
    }))
}

fn header(ctx: &Ctx) -> Row {
  let label = |key: &str| Text::new(&ctx.t(key)).variant(SecretsText::ColumnHeading.role());
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEADER_HEIGHT))
    .padding(Padding::horizontal(SecretsSpacing::Cell.role()))
    .spacing(SecretsSpacing::Cell.role())
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(label("secrets.columns.name").flex(1.0))
    .child(label("secrets.columns.kind").width(Dimension::Px(150.0)))
    .child(label("secrets.columns.environment").width(Dimension::Px(110.0)))
    .child(label("secrets.columns.usedBy").width(Dimension::Px(120.0)))
    .child(Row::new().width(Dimension::Px(14.0)))
}
#[derive(Clone)]
struct RowProps {
  secret: Secret,
  used: Arc<str>,
  unused: bool,
  selected: Option<SecretId>,
  view: View,
}
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    self.secret == other.secret
      && self.used == other.used
      && self.unused == other.unused
      && self.selected == other.selected
  }
}
impl DevtoolsInspectable for RowProps {}
struct SecretRow;
impl Component for SecretRow {
  type Props = RowProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let secret = &props.secret;
    let name = Row::new()
      .spacing(ExtraSpacing::Loose.role())
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .child(credential_avatar(
        &format!("secret-tile-{}", secret.id()),
        words::glyph(secret.kind()),
        false,
      ))
      .child(
        Truncated::new(
          format!("secret-name-{}", secret.id()),
          secret.name().as_str(),
          SecretsText::Name.role(),
        )
        .mono()
        .render(ctx),
      );
    let text = |value: &str| Text::new(value).variant(TypographyStyle::Description).nowrap();
    let mut row = TableRow::new(format!("secret-row-{}", secret.id()))
      .secret()
      .cell(ColumnWidth::Fill, name)
      .cell(KIND, text(&ctx.t(words::kind_key(secret.kind()))))
      .cell(ENVIRONMENT, text(&ctx.t(words::environment_key(secret.environment()))))
      .cell(
        USED,
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(
            if props.unused { Icon::Minus } else { Icon::Link },
            IconSize::Sm,
            PaletteColor::TextMuted,
          ))
          .child(text(&props.used).color(if props.unused {
            PaletteColor::TextMuted
          } else {
            PaletteColor::TextSecondary
          })),
      );
    let id = secret.id();
    row = row.on_open(move || props.view.inspect(id));
    if props.selected == Some(id) {
      row = row.selected();
    }
    Element::from(row)
  }
}
