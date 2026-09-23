//! Lucide icons, drawn as glyphs of the bundled icon font. Names follow the Pencil icon nodes.

use lurq::{
  app::theme::PaletteColor,
  components::{Text, WindowControlContent},
};

use crate::theme::ExtraTypography;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
  AlertCircle,
  ArrowLeft,
  ArrowRight,
  BookOpen,
  Bot,
  ChevronRight,
  Circle,
  CircleCheck,
  CircleDot,
  CloudOff,
  Copy,
  Download,
  ExternalLink,
  FolderGit,
  FolderLock,
  FolderOpen,
  Folders,
  GitPullRequest,
  KeyRound,
  LayoutDashboard,
  Lock,
  MessagesSquare,
  Minus,
  Plug,
  Plus,
  ServerOff,
  Settings,
  Square,
  Workflow,
  Wrench,
  X,
}

impl Icon {
  /// Code point of the glyph in the bundled `lucide.ttf`.
  const fn glyph(self) -> char {
    match self {
      Self::AlertCircle => '\u{e077}',
      Self::ArrowLeft => '\u{e048}',
      Self::ArrowRight => '\u{e049}',
      Self::BookOpen => '\u{e05f}',
      Self::Bot => '\u{e1bb}',
      Self::ChevronRight => '\u{e06f}',
      Self::Circle => '\u{e076}',
      Self::CircleCheck => '\u{e07c}',
      Self::CircleDot => '\u{e345}',
      Self::CloudOff => '\u{e08d}',
      Self::Copy => '\u{e09e}',
      Self::Download => '\u{e0b2}',
      Self::ExternalLink => '\u{e0b9}',
      Self::FolderGit => '\u{e40a}',
      Self::FolderLock => '\u{e336}',
      Self::FolderOpen => '\u{e247}',
      Self::Folders => '\u{e33f}',
      Self::GitPullRequest => '\u{e0e5}',
      Self::KeyRound => '\u{e4a3}',
      Self::LayoutDashboard => '\u{e1c1}',
      Self::Lock => '\u{e10b}',
      Self::MessagesSquare => '\u{e40d}',
      Self::Minus => '\u{e11c}',
      Self::Plug => '\u{e37f}',
      Self::Plus => '\u{e13d}',
      Self::ServerOff => '\u{e1ea}',
      Self::Settings => '\u{e245}',
      Self::Square => '\u{e167}',
      Self::Workflow => '\u{e425}',
      Self::Wrench => '\u{e1b1}',
      Self::X => '\u{e1b2}',
    }
  }
}

/// Icon sizes used by the design, from the 12px chevron to the 24px recovery panel icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
  Small,
  Button,
  Nav,
  Feature,
  Panel,
}

impl IconSize {
  fn role(self) -> ExtraTypography {
    match self {
      Self::Small => ExtraTypography::IconSmall,
      Self::Button => ExtraTypography::IconButton,
      Self::Nav => ExtraTypography::IconNav,
      Self::Feature => ExtraTypography::IconFeature,
      Self::Panel => ExtraTypography::IconPanel,
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
