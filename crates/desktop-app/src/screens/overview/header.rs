//! The Overview's header (`Header` `cPl0F`), one row in every state: the project's name in 20/600, the checkout in
//! Geist Mono 11.5 `text_muted` (*KON · kontur · master*: the key, then the repository and branch as git names them,
//! read off the UI thread), then Import resources and Open Orchestrator as ghost buttons and New task, primary.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{background, tasks::checkout::Checkout};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ImportTab, OrchestratorChat, ProjectPage, Route},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, OverviewText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

pub(super) fn header(ctx: &mut Ctx, project: &ShownProject) -> Row {
  let id = project.open.project.id();
  let checkout = CheckoutProps {
    repo_root: project.open.repo_root.clone(),
    key: project.open.project.key().as_str().into(),
    revision: project.feed.revision.get(),
  };
  let navigation = Navigation::of(ctx);
  let import = ActionButton::new("overview-import", ButtonKind::Ghost, ctx.t("overview.importResources"))
    .icon(Icon::Download)
    .opens(navigation.to(Route::Project(id, ProjectPage::Import(ImportTab::All))));
  let orchestrator = ActionButton::new(
    "overview-open-orchestrator",
    ButtonKind::Ghost,
    ctx.t("overview.openOrchestrator"),
  )
  .icon(Icon::MessagesSquare)
  .opens(navigation.to(Route::Project(
    id,
    ProjectPage::Orchestrator(OrchestratorChat::default()),
  )));
  let new_task = ActionButton::new("overview-new-task", ButtonKind::Primary, ctx.t("overview.newTask"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::Project(id, ProjectPage::NewTask)));
  Row::new()
    .id("overview-header")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(project.open.project.name().as_str())
        .id("overview-title")
        .variant(OverviewText::Name.role())
        .nowrap(),
    )
    .child(ctx.mount_keyed::<CheckoutLine>("overview-checkout", checkout))
    .child(Spacer::new().flex(1.0))
    .child(import)
    .child(orchestrator)
    .child(new_task)
}

#[derive(Clone, PartialEq)]
struct CheckoutProps {
  repo_root: PathBuf,
  key: Box<str>,
  /// The project feed's revision: git is asked again whenever the project's data is saved.
  revision: u64,
}

impl DevtoolsInspectable for CheckoutProps {}

/// The checkout beside the name: the key alone until git was asked, then with what git named.
struct CheckoutLine;

impl Component for CheckoutLine {
  type Props = CheckoutProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let CheckoutProps {
      repo_root,
      key,
      revision,
    } = ctx.props::<CheckoutProps>().clone();
    let programs = Services::of(ctx).programs;
    let read = ctx.future((repo_root, key.clone(), revision), move |(repo_root, key, _)| {
      let programs = programs.clone();
      async move { background::run(move || Checkout::read(&programs, &repo_root).eyebrow(&key)).await }
    });
    // Git's parts are extra to the key: while they are read, and when the background work did not finish (logged
    // there), the line is the key alone, as when git names neither.
    let line = read
      .state()
      .get()
      .data
      .unwrap_or_else(|| Checkout::default().eyebrow(&key));
    Text::new(&message_text(ctx, &line))
      .id("overview-checkout")
      .variant(ExtraTypography::MonoKey.role())
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
  }
}
