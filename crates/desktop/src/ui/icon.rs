//! Lucide icons, drawn as glyphs of the bundled icon font. Names follow the Pencil icon nodes.

use lurq::{app::theme::PaletteColor, components::Text, node::SelectIcon};

use crate::theme::ExtraTypography;

/// Defines [`Icon`] and the code point of each glyph in the bundled `lucide.ttf`, one line per
/// icon, so the list and the table cannot drift apart. The font names its glyphs as lucide did
/// before its renames, and some old names now belong to other icons: the design's `circle-check`
/// is the font's `check-circle-2`, while its `check-circle` is today's `circle-check-big`;
/// likewise `square-check` is `check-square-2`.
macro_rules! icons {
  ($($name:ident => $glyph:literal,)*) => {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Icon {
      $($name,)*
    }

    impl Icon {
      pub(crate) const fn glyph(self) -> char {
        match self {
          $(Self::$name => $glyph,)*
        }
      }
    }
  };
}

icons! {
  Activity => '\u{e038}',
  AlertCircle => '\u{e077}',
  AlignLeft => '\u{e185}',
  Archive => '\u{e041}',
  ArrowDown => '\u{e042}',
  ArrowDownWideNarrow => '\u{e047}',
  ArrowLeft => '\u{e048}',
  ArrowRight => '\u{e049}',
  ArrowUp => '\u{e04a}',
  ArrowUpRight => '\u{e04d}',
  BookOpen => '\u{e05f}',
  BookmarkCheck => '\u{e51f}',
  BookmarkPlus => '\u{e23d}',
  Bot => '\u{e1bb}',
  Braces => '\u{e36a}',
  ChartColumn => '\u{e2a3}',
  Check => '\u{e06c}',
  ChevronDown => '\u{e06d}',
  ChevronLeft => '\u{e06e}',
  ChevronRight => '\u{e06f}',
  ChevronUp => '\u{e070}',
  ChevronsUpDown => '\u{e211}',
  Circle => '\u{e076}',
  CircleCheck => '\u{e226}',
  CircleDashed => '\u{e4b0}',
  CircleDot => '\u{e345}',
  CircleHelp => '\u{e082}',
  CircleMinus => '\u{e07e}',
  CircleOff => '\u{e401}',
  CircleSlash => '\u{e402}',
  CirclePause => '\u{e07f}',
  CircleX => '\u{e084}',
  Clock => '\u{e087}',
  Clock3 => '\u{e250}',
  Cloud => '\u{e088}',
  CloudOff => '\u{e08d}',
  Code => '\u{e093}',
  CodeXml => '\u{e206}',
  Copy => '\u{e09e}',
  CopyCheck => '\u{e3fb}',
  CornerDownLeft => '\u{e0a1}',
  CornerDownRight => '\u{e0a2}',
  Cpu => '\u{e0a9}',
  Crosshair => '\u{e0ac}',
  Database => '\u{e0ad}',
  Download => '\u{e0b2}',
  Ellipsis => '\u{e0b6}',
  ExternalLink => '\u{e0b9}',
  Eye => '\u{e0ba}',
  EyeOff => '\u{e0bb}',
  FileDiff => '\u{e317}',
  FileMinus => '\u{e0c6}',
  // The font's `file-edit`, today's `file-pen`.
  FilePen => '\u{e31f}',
  FilePenLine => '\u{e320}',
  FilePlus => '\u{e0c9}',
  FileSearch => '\u{e0cb}',
  FileText => '\u{e0cc}',
  Filter => '\u{e0dc}',
  Folder => '\u{e0d7}',
  FolderGit => '\u{e40a}',
  FolderInput => '\u{e334}',
  FolderLock => '\u{e336}',
  FolderOutput => '\u{e337}',
  FolderPen => '\u{e338}',
  GripVertical => '\u{e0eb}',
  FolderOpen => '\u{e247}',
  FolderSearch => '\u{e339}',
  FolderX => '\u{e33e}',
  Folders => '\u{e33f}',
  FormInput => '\u{e21f}',
  Gauge => '\u{e1bf}',
  Gear => '\u{e154}',
  GitBranch => '\u{e0e2}',
  GitCompareArrows => '\u{e553}',
  GitMerge => '\u{e0e4}',
  GitPullRequest => '\u{e0e5}',
  GitPullRequestClosed => '\u{e35a}',
  GitPullRequestDraft => '\u{e35b}',
  Globe => '\u{e0e8}',
  Hand => '\u{e1d7}',
  History => '\u{e1f5}',
  Hourglass => '\u{e296}',
  Inbox => '\u{e0f7}',
  Info => '\u{e0f9}',
  Key => '\u{e0fd}',
  KeyRound => '\u{e4a3}',
  Keyboard => '\u{e284}',
  LayoutDashboard => '\u{e1c1}',
  LayoutTemplate => '\u{e207}',
  Lightbulb => '\u{e1c2}',
  Link => '\u{e102}',
  List => '\u{e106}',
  ListChecks => '\u{e1d0}',
  ListFilter => '\u{e460}',
  ListPlus => '\u{e23f}',
  Loader => '\u{e109}',
  Lock => '\u{e10b}',
  LockKeyhole => '\u{e531}',
  LogIn => '\u{e10d}',
  LogOut => '\u{e10e}',
  Maximize => '\u{e112}',
  MessageCircle => '\u{e116}',
  // The font's `message-circle-question-mark`.
  MessageCircleQuestion => '\u{e568}',
  MessageSquare => '\u{e117}',
  MessageSquareDashed => '\u{e40b}',
  MessageSquareX => '\u{e577}',
  MessagesSquare => '\u{e40d}',
  Minus => '\u{e11c}',
  Monitor => '\u{e11d}',
  MonitorCheck => '\u{e482}',
  OctagonPause => '\u{e21b}',
  PanelRight => '\u{e431}',
  PanelRightClose => '\u{e432}',
  PanelRightOpen => '\u{e434}',
  Paperclip => '\u{e12d}',
  Pause => '\u{e12e}',
  Pencil => '\u{e1f9}',
  Pin => '\u{e259}',
  Play => '\u{e13c}',
  Plug => '\u{e37f}',
  Plus => '\u{e13d}',
  Power => '\u{e140}',
  Radio => '\u{e142}',
  RefreshCw => '\u{e145}',
  Repeat => '\u{e146}',
  Rocket => '\u{e286}',
  RotateCcw => '\u{e148}',
  Save => '\u{e14d}',
  ScanEye => '\u{e536}',
  Search => '\u{e151}',
  SearchX => '\u{e4ad}',
  ServerOff => '\u{e1ea}',
  Server => '\u{e153}',
  Settings => '\u{e245}',
  Shield => '\u{e158}',
  ShieldAlert => '\u{e1fe}',
  ShieldCheck => '\u{e1ff}',
  ShieldOff => '\u{e15a}',
  SlidersHorizontal => '\u{e29a}',
  SignalHigh => '\u{e260}',
  SignalLow => '\u{e261}',
  SignalMedium => '\u{e262}',
  Table => '\u{e17d}',
  Tag => '\u{e17f}',
  Tags => '\u{e35c}',
  Sparkles => '\u{e412}',
  Square => '\u{e167}',
  SquareCheck => '\u{e559}',
  SquareMinus => '\u{e171}',
  SquarePlus => '\u{e173}',
  Star => '\u{e176}',
  Terminal => '\u{e181}',
  Timer => '\u{e1e0}',
  Trash => '\u{e18e}',
  TrendingDown => '\u{e190}',
  TrendingUp => '\u{e191}',
  TriangleAlert => '\u{e193}',
  Type => '\u{e198}',
  Undo2 => '\u{e2a1}',
  Unplug => '\u{e45d}',
  Upload => '\u{e19e}',
  User => '\u{e19f}',
  UserCheck => '\u{e1a0}',
  UserCog => '\u{e342}',
  UserPlus => '\u{e1a2}',
  UserX => '\u{e1a3}',
  Users => '\u{e1a4}',
  Wifi => '\u{e1ae}',
  Workflow => '\u{e425}',
  Wrench => '\u{e1b1}',
  X => '\u{e1b2}',
  Zap => '\u{e1b4}',
  AtSign => '\u{e04e}',
  UserMinus => '\u{e1a1}',
  Crown => '\u{e1d6}',
  HardDriveDownload => '\u{e4e5}',
  CloudUpload => '\u{e091}',
  HardDrive => '\u{e0ed}',
  FileSymlink => '\u{e327}',
  UserRoundX => '\u{e46d}',
}

/// Icon sizes used by the design, from the 11px lock of an unavailable option's reason to the
/// 24px recovery panel icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
  Tag,
  Small,
  Reason,
  Marker,
  Dismiss,
  Button,
  Option,
  Nav,
  Notice,
  Feature,
  Panel,
  Large,
  Row,
  Entry,
  Event,
  State,
}

impl IconSize {
  pub(crate) fn role(self) -> ExtraTypography {
    match self {
      Self::Tag => ExtraTypography::IconTag,
      Self::Small => ExtraTypography::IconSmall,
      Self::Reason => ExtraTypography::IconReason,
      Self::Marker => ExtraTypography::IconMarker,
      Self::Dismiss => ExtraTypography::IconDismiss,
      Self::Button => ExtraTypography::IconButton,
      Self::Option => ExtraTypography::IconOption,
      Self::Nav => ExtraTypography::IconNav,
      Self::Notice => ExtraTypography::IconNotice,
      Self::Feature => ExtraTypography::IconFeature,
      Self::Panel => ExtraTypography::IconPanel,
      Self::Large => ExtraTypography::IconLarge,
      Self::Row => ExtraTypography::IconRow,
      Self::Entry => ExtraTypography::IconEntry,
      Self::Event => ExtraTypography::IconEvent,
      Self::State => ExtraTypography::IconState,
    }
  }
}

/// `icon` at `size` in `color`; the loader turns while it is shown.
pub fn icon(icon: Icon, size: IconSize, color: PaletteColor) -> Text {
  let glyph = Text::new(icon.glyph().encode_utf8(&mut [0; 4]))
    .variant(size.role().role())
    .color(color)
    .nowrap();
  match icon {
    Icon::Loader => super::loader::turning(glyph),
    _ => glyph,
  }
}

/// The lucide glyph of a Windows title-bar window control, in the muted text color (`QQmsI`).
#[cfg(not(target_os = "macos"))]
pub fn window_control(icon: Icon) -> Text {
  Text::new(icon.glyph().encode_utf8(&mut [0; 4]))
    .variant(ExtraTypography::IconWindowControl.role())
    .color(PaletteColor::TextSecondary)
    .nowrap()
}

/// A select's chevron or option check (`eASPF`, `ClxwV`), in the color the select passes.
pub fn select_icon(icon: Icon) -> SelectIcon {
  SelectIcon::glyph(
    icon.glyph().encode_utf8(&mut [0; 4]).to_string(),
    ExtraTypography::IconSelect.role(),
  )
}
