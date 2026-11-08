//! Archive in a local resource's panel (`…` in the head of `Agents · Archive · used by · confirmation` L11): the head's
//! menu holds *Archive*, which opens the confirmation ([`confirm`]) over the page. The editors of a profile, tool,
//! instruction and skill ask the same confirmation from their own … menus ([`confirm::question`]); archiving is never
//! refused for being used.

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

/// The … menu of a panel's head for the entry `target` names, with the confirmation it opens.
pub(super) fn menu(ctx: &mut Ctx, target: Target) -> Element {
  let key = format!("local-hub-archive-{:?}", target.what);
  ctx.mount_keyed::<ArchiveMenu>(&key, target)
}

struct ArchiveMenu {
  /// The … menu.
  menu: Signal<bool>,
  anchor: ElementRef,
  /// The confirmation.
  open: Signal<bool>,
}

impl Component for ArchiveMenu {
  type Props = Target;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      menu: ctx.signal(false),
      anchor: ctx.element_ref(),
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let target = ctx.props::<Target>().clone();
    let item = MenuItem::new("local-hub-archive", ctx.t("local_hub.archive.action"), {
      let open = self.open.clone();
      move || open.set(true)
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
    let confirmation = self.open.get().then(|| {
      let (keep, done) = (self.open.clone(), self.open.clone());
      confirm::question(
        ctx,
        Props {
          root: target.root,
          project: target.project,
          what: target.what,
          kind: target.kind,
          name: target.name,
          on_keep: Arc::new(move || keep.set(false)),
          on_archived: Arc::new(move || done.set(false)),
        },
      )
    });
    Column::new()
      .child(button)
      .child(menu_at_end(self.anchor.clone(), self.menu.clone(), panel))
      .with_children(confirmation)
  }
}
