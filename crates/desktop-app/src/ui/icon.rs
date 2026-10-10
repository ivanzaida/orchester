//! Lucide icons, drawn as glyphs of the bundled `lucide.ttf`. Variants carry the design's icon names
//! (`chart-column` is `Icon::ChartColumn`). The list holds every icon of the boards the first dogfood
//! build draws (Overview, Tasks, New task and Edit task, the task page, the app's lifecycle and their
//! component layers), sorted by name.

use lurq::{app::theme::PaletteColor, components::Text};

use super::loader;
use crate::{routes::Screen, theme::IconSize};

/// Defines [`Icon`] and the code point of each glyph in `lucide.ttf`, one line per icon, so the list
/// and the table cannot drift apart. The font names some glyphs as lucide did before its renames:
/// the design's `chart-column` is the font's `bar-chart-3`, `circle-alert` its `alert-circle`,
/// `circle-check` its `check-circle-2`, `clock` its `clock-4`, `file-pen` its `file-edit`, `file-warning` its
/// `file-exclamation-point`, `house` its `home`, `loader-circle` its `loader-2`,
/// `message-circle-question` its `message-circle-question-mark`, `square-plus` its `plus-square` and
/// `triangle-alert` its `alert-triangle`.
macro_rules! icons {
  ($($name:ident => $glyph:literal,)*) => {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Icon {
      $($name,)*
    }

    impl Icon {
      pub const ALL: &[Self] = &[$(Self::$name,)*];

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
  Archive => '\u{e041}',
  ArrowDown => '\u{e042}',
  ArrowRight => '\u{e049}',
  ArrowUp => '\u{e04a}',
  ArrowUpRight => '\u{e04d}',
  Ban => '\u{e051}',
  Bell => '\u{e059}',
  BookOpen => '\u{e05f}',
  Bot => '\u{e1bb}',
  ChartColumn => '\u{e2a3}',
  Check => '\u{e06c}',
  CheckCheck => '\u{e38e}',
  ChevronDown => '\u{e06d}',
  ChevronRight => '\u{e06f}',
  ChevronUp => '\u{e070}',
  ChevronsUpDown => '\u{e211}',
  Circle => '\u{e076}',
  CircleAlert => '\u{e077}',
  CircleCheck => '\u{e226}',
  CircleDashed => '\u{e4b0}',
  CircleDot => '\u{e345}',
  CircleDotDashed => '\u{e4b1}',
  CircleHelp => '\u{e082}',
  CirclePause => '\u{e07f}',
  CirclePlay => '\u{e080}',
  CircleSlash => '\u{e402}',
  CircleStop => '\u{e083}',
  CircleX => '\u{e084}',
  Clock => '\u{e087}',
  Clock3 => '\u{e250}',
  ClockAlert => '\u{e62a}',
  CodeXml => '\u{e206}',
  Coins => '\u{e097}',
  Compass => '\u{e09b}',
  Copy => '\u{e09e}',
  CornerDownLeft => '\u{e0a1}',
  DatabaseZap => '\u{e50b}',
  Download => '\u{e0b2}',
  Equal => '\u{e1bd}',
  Eye => '\u{e0ba}',
  File => '\u{e0c0}',
  FileDiff => '\u{e317}',
  FileMinus => '\u{e0c6}',
  FilePen => '\u{e31f}',
  FilePenLine => '\u{e320}',
  FilePlus => '\u{e0c9}',
  FileText => '\u{e0cc}',
  FileWarning => '\u{e319}',
  FileX => '\u{e0cd}',
  Files => '\u{e0cf}',
  Flag => '\u{e0d1}',
  Folder => '\u{e0d7}',
  FolderGit2 => '\u{e40a}',
  FolderOpen => '\u{e247}',
  FolderSearch => '\u{e339}',
  FolderX => '\u{e33e}',
  Folders => '\u{e33f}',
  Gauge => '\u{e1bf}',
  GitBranch => '\u{e0e2}',
  GitCommitHorizontal => '\u{e0e3}',
  GitCompare => '\u{e359}',
  GitCompareArrows => '\u{e553}',
  GitMerge => '\u{e0e4}',
  GitPullRequest => '\u{e0e5}',
  GraduationCap => '\u{e234}',
  Hammer => '\u{e0ec}',
  Hand => '\u{e1d7}',
  HardDrive => '\u{e0ed}',
  History => '\u{e1f5}',
  Hourglass => '\u{e296}',
  House => '\u{e0f5}',
  Infinity => '\u{e1e7}',
  Info => '\u{e0f9}',
  KeyRound => '\u{e4a3}',
  LayoutDashboard => '\u{e1c1}',
  LayoutGrid => '\u{e0ff}',
  LayoutTemplate => '\u{e207}',
  Library => '\u{e100}',
  Lightbulb => '\u{e1c2}',
  List => '\u{e106}',
  ListChecks => '\u{e1d0}',
  ListCollapse => '\u{e59b}',
  ListFilter => '\u{e460}',
  ListPlus => '\u{e23f}',
  LoaderCircle => '\u{e10a}',
  Lock => '\u{e10b}',
  LogOut => '\u{e10e}',
  Map => '\u{e110}',
  MessageCircle => '\u{e116}',
  MessageCircleQuestion => '\u{e568}',
  MessageSquare => '\u{e117}',
  MessageSquareDashed => '\u{e40b}',
  MessageSquareText => '\u{e575}',
  Minus => '\u{e11c}',
  MousePointerClick => '\u{e120}',
  OctagonPause => '\u{e21b}',
  OctagonX => '\u{e128}',
  PanelRight => '\u{e431}',
  PanelRightClose => '\u{e432}',
  PanelRightOpen => '\u{e434}',
  Paperclip => '\u{e12d}',
  Pause => '\u{e12e}',
  Pencil => '\u{e1f9}',
  PencilLine => '\u{e4f0}',
  Play => '\u{e13c}',
  Plug => '\u{e37f}',
  Plus => '\u{e13d}',
  Power => '\u{e140}',
  Radio => '\u{e142}',
  RadioTower => '\u{e404}',
  Reply => '\u{e22a}',
  RotateCcw => '\u{e148}',
  Save => '\u{e14d}',
  ScanEye => '\u{e536}',
  Scissors => '\u{e14e}',
  Search => '\u{e151}',
  SearchX => '\u{e4ad}',
  Settings => '\u{e154}',
  Shield => '\u{e158}',
  ShieldAlert => '\u{e1fe}',
  ShieldCheck => '\u{e1ff}',
  SignalHigh => '\u{e260}',
  SignalLow => '\u{e261}',
  SignalMedium => '\u{e262}',
  SkipForward => '\u{e160}',
  SlidersHorizontal => '\u{e29a}',
  Sparkles => '\u{e412}',
  Square => '\u{e167}',
  SquarePlus => '\u{e173}',
  Tag => '\u{e17f}',
  Tags => '\u{e35c}',
  Terminal => '\u{e181}',
  Timer => '\u{e1e0}',
  Trash2 => '\u{e18e}',
  TriangleAlert => '\u{e193}',
  Undo2 => '\u{e2a1}',
  Unplug => '\u{e45d}',
  User => '\u{e19f}',
  Users => '\u{e1a4}',
  Workflow => '\u{e425}',
  Wrench => '\u{e1b1}',
  X => '\u{e1b2}',
  Zap => '\u{e1b4}',
}

/// `icon` at `size` in `color`. A loader turns for as long as it is shown (`ui::loader`).
pub fn icon(icon: Icon, size: IconSize, color: impl Into<PaletteColor>) -> Text {
  let text = Text::new(&glyph(icon)).variant(size.role()).color(color.into());
  if icon == Icon::LoaderCircle {
    loader::turning(text)
  } else {
    text
  }
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
