//! The module's *More actions* (`More menu` `foSKQ` in `07 · Modules · states`): a 200 wide menu at the end of the
//! `…` beside *New task in module*: *Edit*, which opens Edit module, and *Close module*, which sets its state to
//! Closed at once (*Reopen module* on a closed one, in the same pattern; the design does not draw it).

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Row,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::module::{Module, ModuleDetails, ModuleState};
use orchester_workspace::modules::data::ModulePage;

use super::save::{SaveAction, SaveModule};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route},
  ui::{
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
  },
};

/// The menu's width (`foSKQ`).
const MENU_WIDTH: f32 = 200.0;

/// Where the menu is open.
#[derive(Clone)]
pub struct MenuPlace {
  pub open: Signal<bool>,
  pub anchor: ElementRef,
}

impl MenuPlace {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }
}

/// The `…` button with its menu while it is open.
pub fn more(
  ctx: &mut Ctx,
  (project, page): (&ShownProject, &ModulePage),
  place: &MenuPlace,
  save: &SaveAction,
) -> Element {
  let open = place.open.clone();
  let button = IconButton::new("module-more", Icon::Ellipsis, ctx.t("modules.page.more"))
    .element(place.anchor.clone())
    .on_click(move || open.update(|open| *open = !*open));
  let shown = place.open.get();
  let button = if shown { button.active() } else { button }.render(ctx);
  let menu = shown.then(|| {
    let panel = panel(ctx, (project, page), &place.open, save);
    menu_at_end(place.anchor.clone(), place.open.clone(), panel)
  });
  Row::new().child(button).with_children(menu).into()
}

fn panel(
  ctx: &mut Ctx,
  (project, page): (&ShownProject, &ModulePage),
  open: &Signal<bool>,
  save: &SaveAction,
) -> Element {
  let navigation = Navigation::of(ctx);
  let module = &page.module;
  let edit = Route::Project(project.open.project.id(), ProjectPage::EditModule(module.id()));
  let edit = MenuItem::opens("module-edit", ctx.t("modules.page.edit"), navigation.to(edit))
    .icon(Icon::Pencil)
    .render(ctx, open);
  let (id, key, glyph, state) = match module.details().state() {
    ModuleState::Open => ("module-close", "modules.page.close", Icon::Archive, ModuleState::Closed),
    ModuleState::Closed => (
      "module-reopen",
      "modules.page.reopen",
      Icon::ArchiveRestore,
      ModuleState::Open,
    ),
  };
  let args = with_state(project, module, state);
  let save = save.clone();
  let close = MenuItem::new(id, ctx.t(key), move || {
    if !save.state().get_untracked().is_pending() {
      save.run(args.clone());
    }
  })
  .icon(glyph)
  .render(ctx, open);
  menu_panel("module-more-panel")
    .width(Dimension::Px(MENU_WIDTH))
    .child(edit)
    .child(close)
    .into()
}

/// The save that sets `module`'s state to `state`, its other details as they are.
fn with_state(project: &ShownProject, module: &Module, state: ModuleState) -> SaveModule {
  let details = module.details();
  let changed = ModuleDetails::new(
    details.name().clone(),
    details.description(),
    (details.start(), details.target()),
    state,
  )
  .expect("a stored module's dates are in order, and its state does not touch them");
  SaveModule {
    root: project.open.repo_root.clone(),
    module: Some(Arc::new(module.clone())),
    details: changed,
  }
}
