//! The stack's page after its services: Slots (`Slots` `Cddys`: *Takes N of M slots*, that a start waits until that
//! many of the computer's *Services at once* are free, and a bar of the computer's slots with those it takes filled),
//! Used by (`Used by` `PAbmX`: the suites that need it, or that none does yet and how agents start it), and the side
//! column's *Dependencies* (`z1aSH`: each service under the need it waits for longest, so as deep as its step, a
//! database container marked so; that each arrow is a need and a crash stops what depends on it).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{limits::ComputerServiceSlots, orchester_tools::OrchesterTool};
use orchester_workspace::agent_testing::stack_editor::order::{Branch, tree};

use super::{
  super::{
    super::kit::{Group, SettingRow},
    editor::aside::card,
    words,
  },
  View,
};
use crate::{
  navigation::Navigation,
  queries::{self, Session, app},
  routes::{ProjectPage, Route, ServicesPage},
  theme::{ExtraSpacing, ExtraTypography, IconSize, ServicesSpacing, ServicesText},
  ui::{
    chip::TagChip,
    icon::{Icon, icon},
  },
};

/// The slots' bar (`Bar` `q9HZT`).
const BAR_WIDTH: f32 = 200.0;
/// A slot of the bar (`Slot` `Q44i10`).
const SLOT_HEIGHT: f32 = 8.0;

/// The Slots group, while the stack's services resolve.
pub(super) fn slots_group(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let count = view.members.as_ref()?.as_ref().ok()?.len();
  let session = Session::of(ctx);
  let limit: Option<ComputerServiceSlots> = queries::state(&ctx.query(app::service_slots(session))).data;
  let (label, hint) = match limit.map(|limit| limit.get().get() as usize) {
    Some(limit) if count > limit => (
      ctx.t_args(
        "project_settings.services.stack.slots.takesOf",
        [("count", count.to_string()), ("limit", limit.to_string())],
      ),
      ctx.t("project_settings.services.stack.slots.tooMany"),
    ),
    Some(limit) => (
      ctx.t_args(
        "project_settings.services.stack.slots.takesOf",
        [("count", count.to_string()), ("limit", limit.to_string())],
      ),
      ctx.t_args(
        "project_settings.services.stack.slots.waits",
        [("count", count.to_string())],
      ),
    ),
    None => (
      ctx.t_args(
        "project_settings.services.stack.slots.takes",
        [("count", count.to_string())],
      ),
      ctx.t_args(
        "project_settings.services.stack.slots.waits",
        [("count", count.to_string())],
      ),
    ),
  };
  let bar = limit.map(|limit| bar(count, limit.get().get() as usize));
  let row = SettingRow::new("settings-stack-slots", label)
    .hint(hint)
    .controls(bar)
    .render(ctx);
  Some(
    Group::new(
      "settings-stack-slots-group",
      ctx.t("project_settings.services.stack.slots.title"),
    )
    .row(row)
    .into(),
  )
}

/// The computer's `limit` slots, the `taken` first filled in `accent`, the others in `border`.
fn bar(taken: usize, limit: usize) -> Element {
  Row::new()
    .id("settings-stack-slots-bar")
    .width(Dimension::Px(BAR_WIDTH))
    .spacing(ServicesSpacing::SlotGap.role())
    .align_items(Alignment::Center)
    .with_children((0..limit).map(|at| {
      let fill = if at < taken {
        PaletteColor::Accent
      } else {
        PaletteColor::Border
      };
      Row::new()
        .id(format!("settings-stack-slot-{at}"))
        .describe("state", if at < taken { "taken" } else { "free" })
        .flex(1.0)
        .height(Dimension::Px(SLOT_HEIGHT))
        .background(fill)
        .rounded(RadiusSize::Sm)
    }))
    .into()
}

/// The Used by group: a row per suite that needs the stack, each opening its page, or that none does yet.
pub(super) fn used_by_group(ctx: &mut Ctx, view: &View, typed: &str) -> Element {
  let name = view.name(typed);
  let used_by = view.stored.used_by(&name);
  let rows: Vec<Element> = if used_by.is_empty() {
    let start = format!("{} {name}", OrchesterTool::StartStack.as_str());
    vec![
      SettingRow::new(
        "settings-stack-used-by-none",
        ctx.t("project_settings.services.stack.usedBy.none"),
      )
      .hint(ctx.t_args("project_settings.services.stack.usedBy.start", [("call", start)]))
      .render(ctx),
    ]
  } else {
    let navigation = Navigation::of(ctx);
    let project = ctx.props::<super::StackProps>().tab.project.open.project.id();
    view
      .stored
      .suites
      .iter()
      .filter(|suite| used_by.contains(&&*suite.name))
      .map(|suite| {
        let id = format!("settings-stack-used-by-{}", suite.name);
        SettingRow::new(id.clone(), suite.name.clone())
          .mono_label()
          .beside(TagChip::new(
            format!("{id}-layer"),
            words::layer(ctx, suite.body.layer()),
          ))
          .control(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted))
          .opens(navigation.to(Route::Project(
            project,
            ProjectPage::Services(ServicesPage::Suite(suite.id)),
          )))
          .render(ctx)
      })
      .collect()
  };
  Group::new(
    "settings-stack-used-by",
    ctx.t("project_settings.services.stack.usedBy.title"),
  )
  .rows(rows)
  .into()
}

/// The *Dependencies* card, while the stack's services resolve.
pub(super) fn dependencies_card(ctx: &Ctx, view: &View) -> Option<Element> {
  let stack = view.stack()?;
  let branches = tree(stack, &view.stored.bodies()).ok()?;
  let lines = branches.iter().map(branch_line);
  let note = Text::new(&ctx.t("project_settings.services.stack.dependencies.note"))
    .id("settings-stack-dependencies-note")
    .variant(ExtraTypography::Note.role())
    .width(Dimension::Pct(100.0));
  Some(card(
    "settings-stack-dependencies",
    &ctx.t("project_settings.services.stack.dependencies.title"),
    lines.chain([Element::from(note)]),
  ))
}

/// A service of the tree: 18 further in per level, a 12 `database` for a database container or a 12
/// `corner-down-right` under its need, and its name in Geist Mono 12 `text_secondary`, 6 apart.
fn branch_line(branch: &Branch) -> Element {
  let glyph = match (branch.database, branch.depth) {
    (true, _) => Icon::Database,
    (false, 0) => Icon::Server,
    (false, _) => Icon::CornerDownRight,
  };
  #[expect(
    clippy::cast_precision_loss,
    reason = "a tree is as deep as a stack has services, far below f32's exact integers"
  )]
  let indent = ServicesSpacing::TreeIndent.value() * branch.depth as f32;
  Row::new()
    .id(format!("settings-stack-dependency-{}", branch.name))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(Dimension::Px(indent)))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("depth", branch.depth.to_string())
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&branch.name)
        .id(format!("settings-stack-dependency-{}-name", branch.name))
        .variant(ServicesText::ChainName.role()),
    )
    .into()
}
