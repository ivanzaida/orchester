//! The Secrets list (`06 Secrets` `bVh1e`, nothing yet `08` `aalCu`): the environments as segments with Import from
//! project and Add secret at their end (`Environments` `JBh7t`), then a group per environment the filter shows, its
//! count after its title, each secret a row with its kind's glyph, its name, what uses it, its masked value with when
//! it was set, and the … that opens it (`DEEPSEEK_API_KEY` `JElwG`); an archived secret says so. A project with no
//! secret, or an environment the filter shows with none, says so in a quiet row. How values are kept is said under the
//! groups.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, dimension::Dimension},
};
use orchester_core::secret::{Environment, Secret};
use orchester_workspace::secrets::data::{ProjectSecrets, SecretUseKind};

use super::{
  state::{EnvironmentChoice, View},
  words,
};
use crate::{
  routes::SecretsPanel,
  screens::project_settings::kit::{EmptyRow, Group, SettingRow},
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::Focus,
    skeleton::{self, Skeleton, fading},
    tabs::{Segment, SegmentFace, segmented},
  },
};

pub(super) fn list(ctx: &mut Ctx, listed: &ProjectSecrets, view: &View) -> Element {
  let chosen = view.environment.get().0;
  let mut column = Column::new()
    .id("secrets-list")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(toolbar(ctx, view, chosen));
  let any = |environment| {
    listed.in_environment(environment).next().is_some() || listed.archived_in(environment).next().is_some()
  };
  if !any(None) {
    column = column.child(project_empty(ctx, view, chosen));
  } else {
    let shown: Vec<Environment> = match chosen {
      Some(environment) => vec![environment],
      None => Environment::ALL
        .into_iter()
        .filter(|environment| any(Some(*environment)))
        .collect(),
    };
    for environment in shown {
      column = column.child(group(ctx, listed, view, environment));
    }
  }
  column
    .child(
      Text::new(&ctx.t("secrets.footer"))
        .id("secrets-handling")
        .variant(ProjectSettingsText::Hint.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// Each secret's bones while they are read: its name, what uses it and its masked value with when it was set.
const READING: [(f32, f32, f32); 4] = [(112.0, 150.0, 130.0), (56.0, 156.0, 138.0), (82.0, 64.0, 96.0), (96.0, 222.0, 0.0)];

/// The list while the secrets are read (`12 · Project settings · Secrets · loading` `IHhqs`): the environments, Import
/// from project and Add secret work, the key glyph and the note stay; the group's name and count and each secret's
/// name, use line and set date are bones, the lower rows fading. *Reading the secrets…* (`secrets.reading`) names it.
pub(super) fn reading(ctx: &mut Ctx, view: &View) -> Element {
  let chosen = view.environment.get().0;
  let rows: Vec<Element> = fading(
    READING
      .iter()
      .enumerate()
      .map(|(at, (name, used, set))| {
        let row = SettingRow::new(format!("secrets-reading-{at}"), "")
          .item()
          .mono_label()
          .label_reading(*name)
          .hint_reading(*used)
          .lead(icon(Icon::KeyRound, IconSize::Md, PaletteColor::TextMuted));
        let row = if *set > 0.0 {
          row.control(skeleton::line(ctx, ExtraTypography::Note.role(), *set))
        } else {
          row
        };
        Column::new().width(Dimension::Pct(100.0)).child(row.render(ctx))
      })
      .collect(),
  )
  .into_iter()
  .map(Element::from)
  .collect();
  let title = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(skeleton::line(ctx, ProjectSettingsText::GroupTitle.role(), 70.0))
    .child(skeleton::line(ctx, ExtraTypography::Note.role(), 50.0));
  let group = Group::new("secrets-reading-group", "").beside(title).rows(rows);
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(group)
    .child(
      Text::new(&ctx.t("secrets.footer"))
        .variant(ProjectSettingsText::Hint.role())
        .width(Dimension::Pct(100.0)),
    );
  Column::new()
    .id("secrets-list")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .child(toolbar(ctx, view, chosen))
    .child(Skeleton::new("secrets-loading", ctx.t("secrets.reading")).show(ctx, bones))
    .into()
}

/// The environments as segments, then Import from project and Add secret.
fn toolbar(ctx: &Ctx, view: &View, chosen: Option<Environment>) -> Row {
  let segment = |environment: Option<Environment>, id: &str, label: Arc<str>| {
    let choice = view.environment.clone();
    Segment::new(
      id.to_owned(),
      SegmentFace::Text(label),
      chosen == environment,
      move || choice.set(EnvironmentChoice(environment)),
    )
  };
  let mut segments = vec![segment(None, "secrets-environment-all", ctx.t("secrets.allShort"))];
  segments.extend(Environment::ALL.into_iter().map(|environment| {
    segment(
      Some(environment),
      &format!("secrets-environment-{}", environment.as_str()),
      ctx.t(words::environment_key(environment)),
    )
  }));
  let add = view.clone();
  Row::new()
    .id("secrets-filter")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(segmented("secrets-environment", segments, Focus::Pointer))
    .child(Spacer::new().flex(1.0))
    .child(
      ActionButton::new("secrets-import", ButtonKind::Ghost, ctx.t("secrets.importFromProject"))
        .icon(Icon::Download)
        .compact()
        .opens(view.import()),
    )
    .child(
      ActionButton::new("secrets-add", ButtonKind::Secondary, ctx.t("secrets.add"))
        .icon(Icon::Plus)
        .on_click(move || add.open(SecretsPanel::Add, None)),
    )
}

/// No secret at all (`Empty · No secrets yet` `MSVAy`), under the environment the filter shows.
fn project_empty(ctx: &Ctx, view: &View, chosen: Option<Environment>) -> Element {
  let add = view.clone();
  let title = chosen.map_or_else(
    || ctx.t("secrets.allEnvironments"),
    |environment| ctx.t(words::environment_key(environment)),
  );
  Group::new("secrets-group", title)
    .row(
      EmptyRow::new(
        "secrets-empty",
        Icon::KeyRound,
        ctx.t("secrets.emptyTitle"),
        ctx.t("secrets.emptyDetail"),
      )
      .action(
        ActionButton::new("secrets-empty-add", ButtonKind::Primary, ctx.t("secrets.add"))
          .icon(Icon::Plus)
          .on_click(move || add.open(SecretsPanel::Add, None)),
      )
      .action(
        ActionButton::new("secrets-empty-import", ButtonKind::Ghost, ctx.t("secrets.import"))
          .icon(Icon::Download)
          .compact()
          .opens(view.import()),
      ),
    )
    .into()
}

/// An environment's group: its secrets, then its archived ones (`OLD_FIGMA_TOKEN` in `06`), or that the filter's
/// environment has none.
fn group(ctx: &mut Ctx, listed: &ProjectSecrets, view: &View, environment: Environment) -> Element {
  let secrets: Vec<Secret> = listed
    .in_environment(Some(environment))
    .chain(listed.archived_in(Some(environment)))
    .cloned()
    .collect();
  let count = ctx.t_args(
    if secrets.len() == 1 {
      "secrets.countOne"
    } else {
      "secrets.count"
    },
    [("count", secrets.len().to_string())],
  );
  let group = Group::new(
    &format!("secrets-group-{}", environment.as_str()),
    ctx.t(words::environment_key(environment)),
  )
  .beside(
    Text::new(&count)
      .id("secrets-count")
      .variant(ExtraTypography::Note.role()),
  );
  if secrets.is_empty() {
    return group
      .row(EmptyRow::new(
        "secrets-environment-empty",
        Icon::KeyRound,
        ctx.t_args(
          "secrets.environmentEmpty",
          [("environment", ctx.t(words::environment_key(environment)))],
        ),
        ctx.t("secrets.environmentEmptyDetail"),
      ))
      .into();
  }
  let rows = secrets
    .into_iter()
    .map(|secret| {
      let props = RowProps {
        used: used_words(ctx, listed, &secret),
        set: words::date(ctx, secret.rotated_at().unwrap_or(secret.created_at())),
        secret,
        view: view.clone(),
      };
      ctx.mount_keyed::<SecretRow>(&format!("secret-row-{}", props.secret.id()), props)
    })
    .collect::<Vec<_>>();
  group.rows(rows).into()
}

/// What uses a secret, as its row says it: the one tool or MCP server by name, how many, or none; an archived one says
/// it gives nothing.
fn used_words(ctx: &Ctx, listed: &ProjectSecrets, secret: &Secret) -> Arc<str> {
  let counts = listed.use_counts(secret.id());
  if secret.is_archived() {
    if counts.total() == 0 {
      return ctx.t("secrets.rowArchivedUnused");
    }
    return ctx.t_args("secrets.rowArchived", [("uses", words::uses(ctx, counts))]);
  }
  let uses: Vec<_> = listed.used_by(secret.id()).collect();
  match uses.as_slice() {
    [] => ctx.t("secrets.rowUnused"),
    [single] => match single.kind {
      SecretUseKind::Tool(_) => ctx.t_args("secrets.rowUsedTool", [("name", single.name.to_string())]),
      SecretUseKind::McpServer(_) => ctx.t_args("secrets.rowUsedServer", [("name", single.name.to_string())]),
    },
    _ => ctx.t_args("secrets.rowUsedBy", [("uses", words::uses(ctx, counts))]),
  }
}

#[derive(Clone)]
struct RowProps {
  secret: Secret,
  used: Arc<str>,
  set: Arc<str>,
  view: View,
}
impl PartialEq for RowProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.secret, &self.used, &self.set) == (&other.secret, &other.used, &other.set)
  }
}
impl DevtoolsInspectable for RowProps {}

/// A secret's row; a press opens its page, where its … menu is. The row ends in that … (`More` `qvfA3`), a 14 glyph in
/// a 24 square.
struct SecretRow;
impl Component for SecretRow {
  type Props = RowProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let secret = &props.secret;
    let id = secret.id();
    let masked = (!secret.is_archived()).then(|| {
      Text::new(&ctx.t_args("secrets.maskedSet", [("date", &*props.set)]))
        .id(format!("secret-mask-{id}"))
        .variant(ExtraTypography::Note.role())
        .nowrap()
    });
    let more = Row::new()
      .id(format!("secret-more-{id}"))
      .width(Dimension::Px(ControlHeight::Sm.value()))
      .height(Dimension::Px(ControlHeight::Sm.value()))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .child(icon(Icon::Ellipsis, IconSize::Md, PaletteColor::TextMuted));
    SettingRow::new(format!("secret-row-{id}"), secret.name().as_str())
      .item()
      .mono_label()
      .lead(icon(words::glyph(secret.kind()), IconSize::Md, PaletteColor::TextMuted))
      .hint(props.used.clone())
      .controls(masked.map(Element::from))
      .control(more)
      .opens(props.view.reveal(id, secret.environment()))
      .render(ctx)
  }
}
