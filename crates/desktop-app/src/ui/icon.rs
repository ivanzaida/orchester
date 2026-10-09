//! Lucide icons, drawn as glyphs of the bundled `lucide.ttf`. Variants carry the design's icon names
//! (`chart-column` is `Icon::ChartColumn`).

use lurq::{app::theme::PaletteColor, components::Text};

use crate::{routes::Screen, theme::IconSize};

/// Defines [`Icon`] and the code point of each glyph in `lucide.ttf`, one line per icon, so the list
/// and the table cannot drift apart. The font names some glyphs as lucide did before its renames: the
/// design's `chart-column` is the font's `bar-chart-3`.
macro_rules! icons {
  ($($name:ident => $glyph:literal,)*) => {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Icon {
      $($name,)*
    }

    impl Icon {
      pub const fn glyph(self) -> char {
        match self {
          $(Self::$name => $glyph,)*
        }
      }
    }
  };
}

icons! {
  AppWindow => '\u{e426}',
  BookOpen => '\u{e05f}',
  Bot => '\u{e1bb}',
  ChartColumn => '\u{e2a3}',
  ChevronDown => '\u{e06d}',
  ChevronsUpDown => '\u{e211}',
  CircleAlert => '\u{e077}',
  CircleHelp => '\u{e082}',
  Copy => '\u{e09e}',
  FileText => '\u{e0cc}',
  FolderOpen => '\u{e247}',
  FolderX => '\u{e33e}',
  Folders => '\u{e33f}',
  GitPullRequest => '\u{e0e5}',
  GraduationCap => '\u{e234}',
  KeyRound => '\u{e4a3}',
  LayoutDashboard => '\u{e1c1}',
  ListChecks => '\u{e1d0}',
  ListPlus => '\u{e23f}',
  LogOut => '\u{e10e}',
  Minus => '\u{e11c}',
  MousePointerClick => '\u{e120}',
  Pencil => '\u{e1f9}',
  Plug => '\u{e37f}',
  Plus => '\u{e13d}',
  RadioTower => '\u{e404}',
  Settings => '\u{e154}',
  SlidersHorizontal => '\u{e29a}',
  Sparkles => '\u{e412}',
  Square => '\u{e167}',
  Workflow => '\u{e425}',
  Wrench => '\u{e1b1}',
  X => '\u{e1b2}',
}

pub fn icon(icon: Icon, size: IconSize, color: PaletteColor) -> Text {
  Text::new(&glyph(icon)).variant(size.role()).color(color)
}

pub fn glyph(icon: Icon) -> String {
  icon.glyph().to_string()
}

/// The icon of a screen in the sidebar, the tabs and its placeholder (`Nav · *` in `q9mf0k`; the tabs of
/// New task and Edit task in `s7jKkA` and `Qp1B0`).
pub const fn screen_icon(screen: Screen) -> Icon {
  match screen {
    Screen::AllProjects => Icon::Folders,
    Screen::Settings => Icon::Settings,
    Screen::Overview => Icon::LayoutDashboard,
    Screen::Tasks | Screen::Task => Icon::ListChecks,
    Screen::NewTask => Icon::ListPlus,
    Screen::EditTask => Icon::Pencil,
    Screen::PullRequests => Icon::GitPullRequest,
    Screen::Usage => Icon::ChartColumn,
    Screen::Pipelines => Icon::Workflow,
    Screen::Agents => Icon::Bot,
    Screen::Instructions => Icon::BookOpen,
    Screen::Skills => Icon::GraduationCap,
    Screen::Tools => Icon::Wrench,
    Screen::Secrets => Icon::KeyRound,
    Screen::Connections => Icon::Plug,
    Screen::ProjectSettings => Icon::SlidersHorizontal,
  }
}
