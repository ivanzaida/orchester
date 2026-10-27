//! Lucide icons, drawn as glyphs of the bundled `lucide.ttf`. Variants carry the design's icon names
//! (`chart-column` is `Icon::ChartColumn`). The list holds every icon of the boards the first dogfood
//! build draws (Overview, Tasks, New task and Edit task, the task page, the app's lifecycle and their
//! component layers), sorted by name.

use lurq::{app::theme::PaletteColor, components::Text};
use orchester_workspace::highlight::language;

use super::loader;
use crate::{routes::Screen, theme::IconSize};

/// Defines [`Icon`] and the code point of each glyph in `lucide.ttf`, one line per icon, so the list
/// and the table cannot drift apart. The font names some glyphs as lucide did before its renames:
/// the design's `chart-column` is the font's `bar-chart-3`, `circle-alert` its `alert-circle`,
/// `circle-check` its `check-circle-2`, `clock` its `clock-4`, `file-pen` its `file-edit`, `file-warning` its
/// `file-exclamation-point`, `house` its `home`, `loader-circle` its `loader-2`,
/// `message-circle-question` its `message-circle-question-mark`, `plug-zap` its `plug-zap-2`,
/// `square-arrow-out-up-right` its `external-link` ([`Icon::ExternalLink`]), `square-plus` its `plus-square`,
/// `shield-x` its `shield-close`, `triangle-alert` its `alert-triangle`, `user-round-check` its `user-check` and
/// `user-round-cog` its `user-cog`.
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
  Activity => '\u{e038}',
  AppWindow => '\u{e426}',
  Archive => '\u{e041}',
  ArchiveRestore => '\u{e2cd}',
  ArrowDown => '\u{e042}',
  ArrowLeft => '\u{e048}',
  ArrowRight => '\u{e049}',
  ArrowRightLeft => '\u{e417}',
  ArrowUp => '\u{e04a}',
  ArrowUpRight => '\u{e04d}',
  ArrowUpToLine => '\u{e45b}',
  Ban => '\u{e051}',
  Bell => '\u{e059}',
  Binary => '\u{e1f2}',
  BookOpen => '\u{e05f}',
  Bot => '\u{e1bb}',
  // Also the design's `text-select`, which `lucide.ttf` has no glyph for (`wecdw`): the same dashed box.
  BoxSelect => '\u{e1cb}',
  Braces => '\u{e36a}',
  Brain => '\u{e3c6}',
  CalendarClock => '\u{e304}',
  CalendarDays => '\u{e2b9}',
  CaseSensitive => '\u{e3d9}',
  ChartColumn => '\u{e2a3}',
  Check => '\u{e06c}',
  CheckCheck => '\u{e38e}',
  ChevronDown => '\u{e06d}',
  ChevronLeft => '\u{e06e}',
  ChevronRight => '\u{e06f}',
  ChevronUp => '\u{e070}',
  ChevronsDownUp => '\u{e228}',
  ChevronsUp => '\u{e074}',
  ChevronsUpDown => '\u{e211}',
  Circle => '\u{e076}',
  CircleAlert => '\u{e077}',
  CircleCheck => '\u{e226}',
  CircleDashed => '\u{e4b0}',
  CircleDot => '\u{e345}',
  CircleDotDashed => '\u{e4b1}',
  CircleHelp => '\u{e082}',
  CircleMinus => '\u{e07e}',
  CircleOff => '\u{e401}',
  CirclePause => '\u{e07f}',
  CirclePlay => '\u{e080}',
  CircleSlash => '\u{e402}',
  CircleStop => '\u{e083}',
  CircleUserRound => '\u{e462}',
  CircleX => '\u{e084}',
  Clipboard => '\u{e085}',
  ClipboardCheck => '\u{e219}',
  Clock => '\u{e087}',
  Clock3 => '\u{e250}',
  ClockAlert => '\u{e62a}',
  Cloud => '\u{e088}',
  CloudCheck => '\u{e66e}',
  CloudOff => '\u{e08d}',
  CloudUpload => '\u{e091}',
  CodeXml => '\u{e206}',
  Coins => '\u{e097}',
  Compass => '\u{e09b}',
  Copy => '\u{e09e}',
  CornerDownLeft => '\u{e0a1}',
  CornerDownRight => '\u{e0a2}',
  Cpu => '\u{e0a9}',
  Crosshair => '\u{e0ac}',
  Database => '\u{e0ad}',
  DatabaseZap => '\u{e50b}',
  Download => '\u{e0b2}',
  Ellipsis => '\u{e0b6}',
  Equal => '\u{e1bd}',
  ExternalLink => '\u{e0b9}',
  Eye => '\u{e0ba}',
  EyeOff => '\u{e0bb}',
  File => '\u{e0c0}',
  FileCode => '\u{e0c3}',
  FileDiff => '\u{e317}',
  FileImage => '\u{e31c}',
  FileMinus => '\u{e0c6}',
  FilePen => '\u{e31f}',
  FilePenLine => '\u{e320}',
  FilePlus => '\u{e0c9}',
  FileText => '\u{e0cc}',
  FileWarning => '\u{e319}',
  FileX => '\u{e0cd}',
  Files => '\u{e0cf}',
  Flag => '\u{e0d1}',
  FlaskConical => '\u{e0d5}',
  Folder => '\u{e0d7}',
  FolderCheck => '\u{e32e}',
  FolderGit2 => '\u{e40a}',
  FolderLock => '\u{e336}',
  FolderOpen => '\u{e247}',
  FolderPlus => '\u{e0d9}',
  FolderSearch => '\u{e339}',
  FolderTree => '\u{e33c}',
  FolderX => '\u{e33e}',
  Folders => '\u{e33f}',
  Gauge => '\u{e1bf}',
  GitBranch => '\u{e0e2}',
  GitBranchPlus => '\u{e1f4}',
  GitCommitHorizontal => '\u{e0e3}',
  GitCompare => '\u{e359}',
  GitCompareArrows => '\u{e553}',
  GitMerge => '\u{e0e4}',
  GitPullRequest => '\u{e0e5}',
  GitPullRequestClosed => '\u{e35a}',
  GitPullRequestDraft => '\u{e35b}',
  Globe => '\u{e0e8}',
  GraduationCap => '\u{e234}',
  Hammer => '\u{e0ec}',
  Hand => '\u{e1d7}',
  HardDrive => '\u{e0ed}',
  Hash => '\u{e0ef}',
  History => '\u{e1f5}',
  Hourglass => '\u{e296}',
  House => '\u{e0f5}',
  Inbox => '\u{e0f7}',
  Infinity => '\u{e1e7}',
  Info => '\u{e0f9}',
  Key => '\u{e0fd}',
  KeyRound => '\u{e4a3}',
  Laptop => '\u{e1cd}',
  Layers2 => '\u{e52a}',
  LayoutDashboard => '\u{e1c1}',
  LayoutGrid => '\u{e0ff}',
  LayoutTemplate => '\u{e207}',
  Library => '\u{e100}',
  Lightbulb => '\u{e1c2}',
  Link => '\u{e102}',
  List => '\u{e106}',
  ListChecks => '\u{e1d0}',
  ListCollapse => '\u{e59b}',
  ListFilter => '\u{e460}',
  ListOrdered => '\u{e1d1}',
  ListPlus => '\u{e23f}',
  ListTodo => '\u{e4c3}',
  LoaderCircle => '\u{e10a}',
  Lock => '\u{e10b}',
  LockKeyhole => '\u{e531}',
  LogIn => '\u{e10d}',
  LogOut => '\u{e10e}',
  Mail => '\u{e10f}',
  MailWarning => '\u{e367}',
  Map => '\u{e110}',
  MessageCircle => '\u{e116}',
  MessageCircleQuestion => '\u{e568}',
  MessageSquare => '\u{e117}',
  MessageSquareDashed => '\u{e40b}',
  MessageSquarePlus => '\u{e40c}',
  MessageSquareReply => '\u{e573}',
  MessageSquareText => '\u{e575}',
  MessagesSquare => '\u{e40d}',
  Minus => '\u{e11c}',
  Monitor => '\u{e11d}',
  MonitorPlay => '\u{e485}',
  Moon => '\u{e11e}',
  MousePointerClick => '\u{e120}',
  Newspaper => '\u{e348}',
  OctagonPause => '\u{e21b}',
  OctagonX => '\u{e128}',
  PackageCheck => '\u{e266}',
  PackageOpen => '\u{e2cc}',
  PanelRight => '\u{e431}',
  PanelRightClose => '\u{e432}',
  PanelRightOpen => '\u{e434}',
  Paperclip => '\u{e12d}',
  Pause => '\u{e12e}',
  Pencil => '\u{e1f9}',
  PencilLine => '\u{e4f0}',
  Pin => '\u{e259}',
  PinOff => '\u{e2b6}',
  Play => '\u{e13c}',
  Plug => '\u{e37f}',
  PlugZap => '\u{e45c}',
  Plus => '\u{e13d}',
  Power => '\u{e140}',
  Radio => '\u{e142}',
  RadioTower => '\u{e404}',
  RectangleEllipsis => '\u{e21f}',
  RefreshCw => '\u{e145}',
  Replace => '\u{e3db}',
  Reply => '\u{e22a}',
  Rocket => '\u{e286}',
  RotateCcw => '\u{e148}',
  RotateCw => '\u{e149}',
  Route => '\u{e53e}',
  Save => '\u{e14d}',
  ScanEye => '\u{e536}',
  Scissors => '\u{e14e}',
  ScrollText => '\u{e45f}',
  Search => '\u{e151}',
  SearchX => '\u{e4ad}',
  Send => '\u{e152}',
  Server => '\u{e153}',
  ServerCrash => '\u{e1e9}',
  Settings => '\u{e154}',
  Shield => '\u{e158}',
  ShieldAlert => '\u{e1fe}',
  ShieldCheck => '\u{e1ff}',
  ShieldOff => '\u{e15a}',
  ShieldX => '\u{e200}',
  Signal => '\u{e25f}',
  SignalHigh => '\u{e260}',
  SignalLow => '\u{e261}',
  SignalMedium => '\u{e262}',
  Signpost => '\u{e540}',
  SkipForward => '\u{e160}',
  SlidersHorizontal => '\u{e29a}',
  Smartphone => '\u{e163}',
  Sparkle => '\u{e47e}',
  Sparkles => '\u{e412}',
  Square => '\u{e167}',
  // The font's `edit`, which lucide renamed `square-pen`.
  SquarePen => '\u{e172}',
  SquarePlus => '\u{e173}',
  SquareTerminal => '\u{e20a}',
  Sun => '\u{e178}',
  Table => '\u{e17d}',
  Tag => '\u{e17f}',
  Tags => '\u{e35c}',
  Target => '\u{e180}',
  Terminal => '\u{e181}',
  TextCursorInput => '\u{e265}',
  Timer => '\u{e1e0}',
  TimerOff => '\u{e249}',
  Trash2 => '\u{e18e}',
  TrendingDown => '\u{e190}',
  TrendingUp => '\u{e191}',
  TriangleAlert => '\u{e193}',
  Undo2 => '\u{e2a1}',
  Unlink => '\u{e19c}',
  Unplug => '\u{e45d}',
  Upload => '\u{e19e}',
  User => '\u{e19f}',
  UserLock => '\u{e660}',
  // The font's `user-2`, which lucide renamed `user-round`.
  UserRound => '\u{e468}',
  UserRoundCheck => '\u{e1a0}',
  UserRoundCog => '\u{e342}',
  UserX => '\u{e1a3}',
  Users => '\u{e1a4}',
  Waypoints => '\u{e542}',
  Wifi => '\u{e1ae}',
  WifiOff => '\u{e1af}',
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

/// Extensions of the files shown as images.
const IMAGES: [&str; 9] = ["png", "jpg", "jpeg", "gif", "ico", "bmp", "webp", "svg", "icns"];

/// The icon of the file at `path` in the code viewer and its tab: an image, code a grammar reads, or text.
pub fn file_icon(path: &str) -> Icon {
  let extension = path.rsplit_once('.').map_or("", |(_, extension)| extension);
  if IMAGES.iter().any(|image| image.eq_ignore_ascii_case(extension)) {
    return Icon::FileImage;
  }
  match language(path) {
    Some("Markdown" | "Plain Text") | None => Icon::FileText,
    Some(_) => Icon::FileCode,
  }
}

/// The icon of a screen in the sidebar, the tabs and its placeholder (`Nav · *` in `q9mf0k`; the tabs of
/// New task and Edit task in `s7jKkA` and `Qp1B0`).
pub const fn screen_icon(screen: Screen) -> Icon {
  match screen {
    Screen::AllProjects => Icon::Folders,
    Screen::AddProject => Icon::Plus,
    Screen::Settings => Icon::Settings,
    Screen::Overview => Icon::LayoutDashboard,
    Screen::Orchestrator | Screen::OrchestratorSettings => Icon::MessagesSquare,
    Screen::Tasks | Screen::Task => Icon::ListChecks,
    Screen::NewTask => Icon::ListPlus,
    Screen::EditTask => Icon::Pencil,
    Screen::Modules => Icon::Target,
    Screen::MergeQueue => Icon::GitMerge,
    Screen::PullRequests => Icon::GitPullRequest,
    Screen::Usage => Icon::ChartColumn,
    Screen::Pipelines => Icon::Workflow,
    Screen::Agents => Icon::Bot,
    Screen::Instructions => Icon::BookOpen,
    Screen::Memory => Icon::Brain,
    Screen::Skills => Icon::GraduationCap,
    Screen::Tools => Icon::Wrench,
    Screen::Secrets => Icon::KeyRound,
    Screen::Connections => Icon::Plug,
    Screen::NewConnection => Icon::Plus,
    Screen::EditConnection => Icon::Pencil,
    Screen::ProjectSettings => Icon::SlidersHorizontal,
    Screen::Files => Icon::FolderTree,
    Screen::ImportResources => Icon::Download,
  }
}

/// Outlined raised icon tile in Skills guidance, empty states and reference rows.
pub fn marker(mark: Icon, side: f32) -> lurq::components::Row {
  use lurq::{
    app::theme::{BorderSize, RadiusSize},
    layout::{Alignment, layout_kind::Justify},
    node::dimension::Dimension,
  };
  lurq::components::Row::new()
    .width(Dimension::Px(side))
    .height(Dimension::Px(side))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .rounded(RadiusSize::Md)
    .child(icon(mark, IconSize::Base, PaletteColor::TextMuted))
}
