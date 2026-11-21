//! The theme's icon set, which lurq's components (`lurq::ui`) draw their icons from by name: every icon of the app's
//! `lucide.ttf` under its design name, and lurq's names for icons the app draws under lucide's own, so that a lurq
//! component draws the app's glyph. lurq's other icons stay its own font's.

use lurq::app::theme::{Theme, ThemeIcons};

use super::fonts::ICONS;
use crate::ui::icon::Icon;

/// lurq's names (`lurq::ui::icons`) of icons the app has under lucide's name.
const LURQ_NAMES: [(&str, Icon); 30] = [
  ("alert-circle", Icon::CircleAlert),
  ("check-circle", Icon::CircleCheck),
  ("close", Icon::X),
  ("close-circle", Icon::CircleX),
  ("branch", Icon::GitBranch),
  ("pull-request", Icon::GitPullRequest),
  ("trash", Icon::Trash2),
  ("warning", Icon::TriangleAlert),
  ("refresh", Icon::RefreshCw),
  ("more-horizontal", Icon::Ellipsis),
  ("spinner", Icon::LoaderCircle),
  ("filter", Icon::ListFilter),
  ("help-circle", Icon::CircleHelp),
  ("home", Icon::House),
  ("undo", Icon::Undo2),
  ("sliders", Icon::SlidersHorizontal),
  ("code", Icon::CodeXml),
  ("dashboard", Icon::LayoutDashboard),
  ("chevrons-vertical", Icon::ChevronsUpDown),
  ("compose", Icon::SquarePen),
  ("enter", Icon::CornerDownLeft),
  ("window", Icon::AppWindow),
  ("bolt", Icon::Zap),
  ("sign-out", Icon::LogOut),
  ("search-close", Icon::SearchX),
  ("cut", Icon::Scissors),
  ("arrow-left-right", Icon::ArrowRightLeft),
  ("dot-circle", Icon::CircleDot),
  ("play-circle", Icon::CirclePlay),
  ("list-check", Icon::ListChecks),
];

pub(super) fn install(theme: &Theme) {
  let mut icons = ThemeIcons::lurq();
  icons.set_font(ICONS, Icon::ALL.iter().map(|icon| (icon.name(), icon.glyph())));
  icons.set_font(ICONS, LURQ_NAMES.iter().map(|(name, icon)| (*name, icon.glyph())));
  theme.set_icons(icons);
}
