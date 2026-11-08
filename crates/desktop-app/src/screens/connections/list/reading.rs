//! Connections while they are read (`11 · Project settings · Connections · loading` `dlBtk`): Add provider works, the
//! Agent providers and GitHub headings stay, and GitHub's row keeps its kind's tile; each connection's mark, name,
//! program or account line and status are bones in the rows' own geometry, the lower group fading. *Reading the
//! connections…* (`connections.reading`) names it.

use lurq::{
  app::{ctx::Ctx, theme::RadiusSize},
  components::Column,
  node::{Element, dimension::Dimension},
};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{NewConnection, ProjectPage, Route},
  screens::{
    connections::words::GITHUB_GLYPH,
    project_settings::kit::{Group, SettingRow, tile},
  },
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    skeleton::{self, Skeleton, bone, fading},
  },
};

/// A row's tile while its kind is read (`Tile` `BpusX`).
const TILE: f32 = 28.0;
/// Each agent provider's bones: its name, its line and its status.
const PROVIDERS: [(f32, f32, f32); 4] = [
  (124.0, 302.0, 58.0),
  (72.0, 320.0, 62.0),
  (50.0, 422.0, 46.0),
  (80.0, 146.0, 76.0),
];

/// The groups while the connections are read.
pub(super) fn groups(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let providers = PROVIDERS
    .iter()
    .enumerate()
    .map(|(at, (name, line, status))| {
      let lead = bone(TILE, TILE).rounded(RadiusSize::Md);
      row(
        ctx,
        (&format!("connections-reading-{at}"), lead.into()),
        (*name, *line),
        skeleton::status(ctx, *status),
      )
    })
    .collect::<Vec<_>>();
  let github = vec![row(
    ctx,
    ("connections-reading-github", tile(ctx, GITHUB_GLYPH).into()),
    (52.0, 212.0),
    skeleton::status(ctx, 46.0),
  )];
  let agents_add = ActionButton::new(
    "connections-add-agent",
    ButtonKind::Ghost,
    ctx.t("connections.groups.addProvider"),
  )
  .icon(Icon::Plus)
  .in_heading()
  .opens(navigation.to(Route::Project(
    id,
    ProjectPage::NewConnection(NewConnection::AgentProvider),
  )));
  let groups = vec![
    Column::new().width(Dimension::Pct(100.0)).child(
      Group::new("connections-reading-agents", ctx.t("connections.groups.agentProviders"))
        .end(agents_add)
        .rows(providers),
    ),
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(Group::new("connections-reading-github", ctx.t("connections.groups.github")).rows(github)),
  ];
  let bones = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Roomy.role())
    .with_children(fading(groups));
  Skeleton::new("connections-loading", ctx.t("connections.reading")).show(ctx, bones)
}

/// A connection's row in bones: its tile, its name and line, `end` and the chevron.
fn row(ctx: &mut Ctx, (id, lead): (&str, Element), (name, line): (f32, f32), end: impl Into<Element>) -> Element {
  SettingRow::new(id.to_owned(), "")
    .item()
    .label_reading(name)
    .hint_reading(line)
    .lead(lead)
    .control(end)
    .control(skeleton::chevron())
    .render(ctx)
}
