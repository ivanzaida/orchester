//! Lucide icons, drawn as glyphs of the bundled icon font. Names follow the Pencil icon nodes.

use lurq::{
  app::theme::PaletteColor,
  components::{Text, WindowControlContent},
  node::SelectIcon,
};

use crate::theme::ExtraTypography;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
  Activity,
  AlertCircle,
  ArrowLeft,
  ArrowRight,
  BookOpen,
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Circle,
  CircleCheck,
  CircleDashed,
  CircleDot,
  CircleHelp,
  Clock3,
  CloudOff,
  Code,
  Copy,
  Download,
  ExternalLink,
  FilePenLine,
  FileText,
  FolderGit,
  FolderLock,
  FolderOpen,
  FolderSearch,
  Folders,
  GitPullRequest,
  History,
  Info,
  KeyRound,
  LayoutDashboard,
  LayoutTemplate,
  ListChecks,
  ListPlus,
  Loader,
  Lock,
  Maximize,
  MessageCircle,
  MessagesSquare,
  Minus,
  Pencil,
  Pin,
  Play,
  Plug,
  Plus,
  Power,
  RefreshCw,
  Repeat,
  Save,
  ScanEye,
  Search,
  ServerOff,
  ShieldCheck,
  Settings,
  SlidersHorizontal,
  Sparkles,
  Square,
  Terminal,
  Trash,
  Unplug,
  Upload,
  UserX,
  Workflow,
  Wrench,
  X,
}

impl Icon {
  /// Code point of the glyph in the bundled `lucide.ttf`.
  const fn glyph(self) -> char {
    match self {
      Self::Activity => '\u{e038}',
      Self::AlertCircle => '\u{e077}',
      Self::ArrowLeft => '\u{e048}',
      Self::ArrowRight => '\u{e049}',
      Self::BookOpen => '\u{e05f}',
      Self::Bot => '\u{e1bb}',
      Self::ChevronRight => '\u{e06f}',
      Self::Circle => '\u{e076}',
      Self::CircleCheck => '\u{e07c}',
      Self::Check => '\u{e06c}',
      Self::CircleDot => '\u{e345}',
      Self::CircleHelp => '\u{e082}',
      Self::CloudOff => '\u{e08d}',
      Self::Copy => '\u{e09e}',
      Self::Download => '\u{e0b2}',
      Self::FilePenLine => '\u{e320}',
      Self::FileText => '\u{e0cc}',
      Self::ExternalLink => '\u{e0b9}',
      Self::FolderGit => '\u{e40a}',
      Self::FolderLock => '\u{e336}',
      Self::FolderOpen => '\u{e247}',
      Self::FolderSearch => '\u{e339}',
      Self::Folders => '\u{e33f}',
      Self::GitPullRequest => '\u{e0e5}',
      Self::History => '\u{e1f5}',
      Self::KeyRound => '\u{e4a3}',
      Self::LayoutDashboard => '\u{e1c1}',
      Self::Loader => '\u{e109}',
      Self::Lock => '\u{e10b}',
      Self::MessagesSquare => '\u{e40d}',
      Self::Minus => '\u{e11c}',
      Self::Plug => '\u{e37f}',
      Self::Plus => '\u{e13d}',
      Self::Power => '\u{e140}',
      Self::RefreshCw => '\u{e145}',
      Self::Save => '\u{e14d}',
      Self::ServerOff => '\u{e1ea}',
      Self::ShieldCheck => '\u{e1ff}',
      Self::Settings => '\u{e245}',
      Self::Sparkles => '\u{e412}',
      Self::Square => '\u{e167}',
      Self::Unplug => '\u{e45d}',
      Self::Workflow => '\u{e425}',
      Self::Wrench => '\u{e1b1}',
      Self::ChevronDown => '\u{e06d}',
      Self::ChevronUp => '\u{e070}',
      Self::MessageCircle => '\u{e116}',
      Self::Clock3 => '\u{e250}',
      Self::ListPlus => '\u{e23f}',
      Self::Pencil => '\u{e1f9}',
      Self::Play => '\u{e13c}',
      Self::Search => '\u{e151}',
      Self::CircleDashed => '\u{e4b0}',
      Self::Code => '\u{e093}',
      Self::Info => '\u{e0f9}',
      Self::LayoutTemplate => '\u{e207}',
      Self::ListChecks => '\u{e1d0}',
      Self::Maximize => '\u{e112}',
      Self::Pin => '\u{e259}',
      Self::Repeat => '\u{e146}',
      Self::ScanEye => '\u{e536}',
      Self::SlidersHorizontal => '\u{e29a}',
      Self::Terminal => '\u{e181}',
      Self::Trash => '\u{e18e}',
      Self::Upload => '\u{e19e}',
      Self::UserX => '\u{e1a3}',
      Self::X => '\u{e1b2}',
    }
  }
}

/// Icon sizes used by the design, from the 12px chevron to the 24px recovery panel icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
  Small,
  Marker,
  Dismiss,
  Button,
  Option,
  Nav,
  Notice,
  Feature,
  Panel,
  Large,
}

impl IconSize {
  fn role(self) -> ExtraTypography {
    match self {
      Self::Small => ExtraTypography::IconSmall,
      Self::Marker => ExtraTypography::IconMarker,
      Self::Dismiss => ExtraTypography::IconDismiss,
      Self::Button => ExtraTypography::IconButton,
      Self::Option => ExtraTypography::IconOption,
      Self::Nav => ExtraTypography::IconNav,
      Self::Notice => ExtraTypography::IconNotice,
      Self::Feature => ExtraTypography::IconFeature,
      Self::Panel => ExtraTypography::IconPanel,
      Self::Large => ExtraTypography::IconLarge,
    }
  }
}

pub fn icon(icon: Icon, size: IconSize, color: PaletteColor) -> Text {
  Text::new(icon.glyph().encode_utf8(&mut [0; 4]))
    .variant(size.role().role())
    .color(color)
    .nowrap()
}

/// A title-bar window control drawn with a lucide glyph in the color lurq passes for the
/// control's state.
pub fn window_control(icon: Icon) -> WindowControlContent {
  WindowControlContent::element(move |color| {
    Text::new(icon.glyph().encode_utf8(&mut [0; 4]))
      .variant(ExtraTypography::IconWindowControl.role())
      .color(color)
      .nowrap()
      .into()
  })
}

/// A select's chevron or option check (`eASPF`, `ClxwV`), in the color the select passes.
pub fn select_icon(icon: Icon) -> SelectIcon {
  SelectIcon::glyph(
    icon.glyph().encode_utf8(&mut [0; 4]).to_string(),
    ExtraTypography::IconSelect.role(),
  )
}
