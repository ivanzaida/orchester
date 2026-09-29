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
      const fn glyph(self) -> char {
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
  ArrowLeft => '\u{e048}',
  ArrowRight => '\u{e049}',
  ArrowUp => '\u{e04a}',
  ArrowUpRight => '\u{e04d}',
  BookOpen => '\u{e05f}',
  Bot => '\u{e1bb}',
  Braces => '\u{e36a}',
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
  CirclePause => '\u{e07f}',
  CircleX => '\u{e084}',
  Clock => '\u{e087}',
  Clock3 => '\u{e250}',
  CloudOff => '\u{e08d}',
  Code => '\u{e093}',
  CodeXml => '\u{e206}',
  Copy => '\u{e09e}',
  Database => '\u{e0ad}',
  Download => '\u{e0b2}',
  ExternalLink => '\u{e0b9}',
  FileDiff => '\u{e317}',
  FileMinus => '\u{e0c6}',
  FilePenLine => '\u{e320}',
  FilePlus => '\u{e0c9}',
  FileSearch => '\u{e0cb}',
  FileText => '\u{e0cc}',
  Folder => '\u{e0d7}',
  FolderGit => '\u{e40a}',
  FolderLock => '\u{e336}',
  FolderOpen => '\u{e247}',
  FolderSearch => '\u{e339}',
  FolderX => '\u{e33e}',
  Folders => '\u{e33f}',
  FormInput => '\u{e21f}',
  Gauge => '\u{e1bf}',
  GitCompareArrows => '\u{e553}',
  GitMerge => '\u{e0e4}',
  GitPullRequest => '\u{e0e5}',
  GitPullRequestClosed => '\u{e35a}',
  GitPullRequestDraft => '\u{e35b}',
  Globe => '\u{e0e8}',
  Hand => '\u{e1d7}',
  History => '\u{e1f5}',
  Inbox => '\u{e0f7}',
  Info => '\u{e0f9}',
  Key => '\u{e0fd}',
  KeyRound => '\u{e4a3}',
  LayoutDashboard => '\u{e1c1}',
  LayoutTemplate => '\u{e207}',
  Lightbulb => '\u{e1c2}',
  Link => '\u{e102}',
  ListChecks => '\u{e1d0}',
  ListPlus => '\u{e23f}',
  Loader => '\u{e109}',
  Lock => '\u{e10b}',
  LockKeyhole => '\u{e531}',
  Maximize => '\u{e112}',
  MessageCircle => '\u{e116}',
  MessageSquare => '\u{e117}',
  MessagesSquare => '\u{e40d}',
  Minus => '\u{e11c}',
  OctagonPause => '\u{e21b}',
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
  Save => '\u{e14d}',
  ScanEye => '\u{e536}',
  Search => '\u{e151}',
  ServerOff => '\u{e1ea}',
  Settings => '\u{e245}',
  Shield => '\u{e158}',
  ShieldCheck => '\u{e1ff}',
  SlidersHorizontal => '\u{e29a}',
  Sparkles => '\u{e412}',
  Square => '\u{e167}',
  SquareCheck => '\u{e559}',
  SquarePlus => '\u{e173}',
  Terminal => '\u{e181}',
  Timer => '\u{e1e0}',
  Trash => '\u{e18e}',
  TriangleAlert => '\u{e193}',
  Type => '\u{e198}',
  Undo2 => '\u{e2a1}',
  Unplug => '\u{e45d}',
  Upload => '\u{e19e}',
  UserCheck => '\u{e1a0}',
  UserCog => '\u{e342}',
  UserX => '\u{e1a3}',
  Workflow => '\u{e425}',
  Wrench => '\u{e1b1}',
  X => '\u{e1b2}',
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
  fn role(self) -> ExtraTypography {
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

pub fn icon(icon: Icon, size: IconSize, color: PaletteColor) -> Text {
  Text::new(icon.glyph().encode_utf8(&mut [0; 4]))
    .variant(size.role().role())
    .color(color)
    .nowrap()
}

/// A Windows title-bar window control drawn with a lucide glyph in the color lurq passes for the
/// control's state.
#[cfg(not(target_os = "macos"))]
pub fn window_control(icon: Icon) -> lurq::components::WindowControlContent {
  lurq::components::WindowControlContent::element(move |color| {
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
