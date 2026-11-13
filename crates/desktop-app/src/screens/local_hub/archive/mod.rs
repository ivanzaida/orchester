//! Archive in a local resource's panel (`…` in the head of `Agents · Archive · used by · confirmation` L11): the head's
//! menu holds *Archive*, which asks the hub to open the confirmation ([`confirm`]) over the page. The hub hosts it
//! ([`Asking`]), not the panel: the entry leaves the lists once it is archived and its panel goes, while the
//! confirmation may still be saying that a run could not be stopped. The editors of a profile, tool, instruction and
//! skill ask the same confirmation from their own … menus ([`confirm::question`]); archiving is never refused for
//! being used.

pub(in crate::screens) mod confirm;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, resource::ResourceKind};
use orchester_workspace::local_archive::Archivable;

use self::confirm::Props;
use crate::{
  screens::hub::page_head::MORE_MENU_WIDTH,
  ui::{
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// What the head's … menu needs to archive the entry it is about.
#[derive(Clone, PartialEq)]
pub(in crate::screens) struct Target {
  pub root: PathBuf,
  pub project: ProjectId,
  pub what: Archivable,
  pub kind: ResourceKind,
  pub name: Arc<str>,
}
impl DevtoolsInspectable for Target {}

/// The entry whose archive confirmation the hub shows, which the hub provides to the panels below it.
#[derive(Clone)]
pub(in crate::screens) struct Asking(Signal<Option<Target>>);

impl PartialEq for Asking {
  fn eq(&self, other: &Self) -> bool {
    self.0.id() == other.0.id()
  }
}

impl Asking {
  pub(in crate::screens) fn new(ctx: &mut Ctx) -> Self {
    Self(ctx.signal(None))
  }

  /// Asks for the confirmation of archiving the entry `target` names.
  pub(in crate::screens) fn ask(&self, target: Target) {
    self.0.set(Some(target));
  }

  /// The confirmation over the page while an entry of a local project asked for it; it ends with the archive or when
  /// it is left.
  pub(in crate::screens) fn question(&self, ctx: &mut Ctx) -> Option<Element> {
    self.question_in(ctx, true)
  }

  /// [`Self::question`] for a project that is `local` or a team's.
  pub(in crate::screens) fn question_in(&self, ctx: &mut Ctx, local: bool) -> Option<Element> {
    let target = self.0.get()?;
    let (keep, done) = (self.0.clone(), self.0.clone());
    Some(confirm::question(
      ctx,
      Props {
        root: target.root,
        project: target.project,
        what: target.what,
        kind: target.kind,
        name: target.name,
        local,
        on_keep: Arc::new(move || keep.set(None)),
        on_archived: Arc::new(move || done.set(None)),
      },
    ))
  }
}

/// The … menu of a panel's head for the entry `target` names; *Archive* asks the hub for the confirmation.
pub(super) fn menu(ctx: &mut Ctx, target: Target) -> Element {
  let key = format!("local-hub-archive-{:?}", target.what);
  ctx.mount_keyed::<ArchiveMenu>(&key, target)
}

struct ArchiveMenu {
  /// The … menu.
  menu: Signal<bool>,
  anchor: ElementRef,
  asking: Asking,
}

impl Component for ArchiveMenu {
  type Props = Target;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      menu: ctx.signal(false),
      anchor: ctx.element_ref(),
      asking: ctx
        .use_context::<Asking>()
        .expect("the hub provides the archive question to the panels it shows"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let target = ctx.props::<Target>().clone();
    let item = MenuItem::new("local-hub-archive", ctx.t("local_hub.archive.action"), {
      let (asking, target) = (self.asking.0.clone(), target);
      move || asking.set(Some(target.clone()))
    })
    .icon(Icon::Archive)
    .render(ctx, &self.menu);
    let toggle = self.menu.clone();
    let button = IconButton::new("local-hub-more", Icon::Ellipsis, ctx.t("local_hub.archive.more"))
      .small()
      .element(self.anchor.clone())
      .on_click(move || toggle.update(|open| *open = !*open))
      .render(ctx);
    let panel = menu_panel("local-hub-more-menu")
      .width(Dimension::Px(MORE_MENU_WIDTH))
      .child(item);
    Column::new()
      .child(button)
      .child(menu_at_end(self.anchor.clone(), self.menu.clone(), panel))
  }
}
