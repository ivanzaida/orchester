use std::collections::HashMap;

use lurq::{
  app::theme::{ThemeTypography, TypographyStyle},
  layout::text_style::{FontFeature, FontFeatures, FontWeight, TextStyle},
  node::color::Color,
};

use super::tokens::{CODE_COMMENT, CODE_TEXT, FONT, ICON_FONT, MONO, MUTED, SELECT_ICON_SIZE, SUBTLE, TEXT};

/// Pencil text styles without a built-in lurq role, registered as `TypographyStyle::Extra`.
/// Use these instead of spelling extra names at call sites.
#[derive(Clone, Copy)]
pub enum ExtraTypography {
  /// Content section heading, e.g. "What changed" (`kiImw`).
  SectionHeading,
  /// Card and panel titles, e.g. "Needs you" (`qlo9F`).
  CardTitle,
  /// Heading of a column of the scheduling review, "Task priority" and "Execution mode" (`Y68u8`,
  /// `TjVjp` in 02.05 `W1Dq6`).
  ReviewHeading,
  /// Small uppercase category label, e.g. "REVIEW PULL REQUEST" (`f1tfh3`).
  Overline,
  /// Large monospaced figure in project metrics (`PwPmH`).
  Metric,
  /// Product name in the Orchester brand (`qudYK`).
  Brand,
  /// Product name heading the setup rail (`PIceE`).
  SetupBrand,
  /// First launch headline (`cmrMC`).
  Display,
  /// First launch introduction under the headline (`oJkK5`).
  Lead,
  /// Spaced uppercase label above a group, e.g. "WELCOME TO ORCHESTER" (`rrrpd`, `M58ysd`, `K1H7ld`).
  Eyebrow,
  /// Step and explanation headings, e.g. "Connect a repository" (`WTDFX` in `Qazxp`, `Qbj0J`, `yhFcb`).
  Subheading,
  /// Numbered step marker, e.g. "01" (`AtKp7` in `Qazxp`).
  StepNumber,
  /// Page title of the setup steps, e.g. "Create your project" (`j6LVi`).
  PageTitle,
  /// Supporting detail under a notice heading or a rail action (`szYuk` in `q5LzI`, `et2jb`).
  Detail,
  /// Smallest supporting line, e.g. runner capacity (`tGGua`).
  Footnote,
  /// Emphasized notice heading and dialog option title (`T9TyR`, `Oq9Jc`).
  NoticeHeading,
  /// Title of a project screen below its topbar, e.g. "Tasks" (`eQKWX`, `E4mnm8`).
  ScreenTitle,
  /// Label above a field of the new task form, e.g. "Task title" (`Pg9E7`).
  FormLabel,
  /// Text of a status label, e.g. "Draft" (`JuW4o` in `DeizF`).
  StatusLabel,
  /// Count beside a navigation item (`LTXft` in `TSJpn`).
  NavCount,
  /// Task brief as it is written (`E9moJ`).
  Brief,
  /// Dialog title (`bhcaX`).
  DialogTitle,
  /// Dialog explanation under its title (`ClBsl`).
  Explanation,
  /// Pencil in a changed-in-draft tag (`zzOmC`), 10px.
  IconTag,
  /// Chevrons and other 12px icons (`B1lur`, `Y9sVM2`).
  IconSmall,
  /// The lock before why an option cannot be chosen (`OoLYh`, `RuJto` in 04.B `W0Owo`), 11px.
  IconReason,
  /// Acceptance criterion marker (`JjM8I`), 13px.
  IconMarker,
  /// Button and secondary navigation icons (`xfnek`, `ssq5w`).
  IconButton,
  /// Primary navigation icons (`sQnmq`).
  IconNav,
  /// Dismiss icon of a notice (`KGM0S`), 14px.
  IconDismiss,
  /// Option icons in a dialog, task selection and task state icons (`dsi3B`, `R7idX`, `UR2gw`), 16px.
  IconOption,
  /// Inline notice icon and the repository of the new task form (`GkfFF` in `q5LzI`, `iuGdG`), 18px.
  IconNotice,
  /// Footer icon on the first launch screen (`OYpN8`).
  IconFeature,
  /// Recovery panel icon (`xEwzT`).
  IconPanel,
  /// Window control icons in the title bar (`QQmsI`), 14px.
  IconWindowControl,
  /// Chevron of a select and check of its selected option (`eASPF`, `ClxwV`), 14px.
  IconSelect,
  /// Spaced mono label on the pipeline canvas, e.g. "ON TASK START" (`wRjZD`, `qwFek`, `RAoRx`).
  CanvasLabel,
  /// Small mono tag beside an inspector title, e.g. "STEP 05" (`kAJZA`, `N2NBl`).
  MonoTag,
  /// Mono value of a key/value property or a slug, e.g. "$1.50" (`yeeIR` in `BzWOd`, `cJsA7`).
  MonoValue,
  /// Emphasized account limit percentage (`e1TK9`, state N).
  MonoLimit,
  /// A path in a field, e.g. the project folder (`ObbY7` in `xZAZc`, `F5KwDR`).
  MonoField,
  /// Step instructions as they are written (`v1Ky8`).
  Instructions,
  /// Short explanation under a field, e.g. "Choose a model here…" (`k66CB`, `Lwlf3`).
  Hint,
  /// A value read back on one line under a field, e.g. "≈ 14M tokens" under the scheduling review's
  /// token cap (`n5Wrl`, `xPwNl` in `W1Dq6`): 10 subtle.
  ReadBack,
  /// Version figure in the publish confirmation, e.g. "v4" (`p8L552`).
  VersionFigure,
  /// Explanation under an empty state title (`xIG73`).
  Summary,
  /// Empty state icon (`Ujxx0`), 22px.
  IconLarge,
  /// Status label text in a list row (`JuW4o` in `v7McTX`).
  CompactStatusLabel,
  /// Title of an inspector panel, e.g. a connection's name (`kU9QO`).
  InspectorTitle,
  /// Title of a pull request list row (`G2kdX` in `QHBNZ`).
  RowTitle,
  /// Leading icon of a pull request list row (`r0ZlXX` in `QHBNZ`), 20px.
  IconRow,
  /// Title of a pull request's review screen (`CNO65` in `Rrmhf`).
  PullRequestTitle,
  /// Mono line under it: number, branches, head, sync (`ADELW`).
  PullRequestContext,
  /// Heading of the changed file list, e.g. "FILES  6" (`UwSNk`).
  FileListHeading,
  /// Name of a changed file in the file list (`qkwbj` in `j9NDM`).
  FileName,
  /// Path of the file a diff shows (`rxiPH`).
  DiffPath,
  /// Old and new line numbers of a diff line (`b8bSop`, `U4wNN` in `vEZ42`).
  DiffLineNumber,
  /// Code of a diff line and its change mark (`DQMrs`, `K3NKFG` in `vEZ42`); tokens take their
  /// `ExtraColor::Code*` color.
  Code,
  /// Output of a command in the agent transcript (`mMUyR` in `N4SlQq`).
  Output,
  /// Glyph of an agent transcript entry (`DQmo9` in `N4SlQq`), 13px.
  IconEntry,
  /// Glyph of an activity event, e.g. a run's step (`xQYdP` in `mBkeJ`), 14px.
  IconEvent,
  /// Icon of an empty, loading or failed state of a tab (`qgZru` in `hig8O`), 20px.
  IconState,
  /// Title of the instruction being edited (`sjJOc` in `yRKeF`), 20/500.
  RuleTitle,
  /// Instruction prompt text, edited and resolved (`g2Tkg`, `iJdsS`): mono 12, 1.8.
  PromptText,
  /// Name of the variable shown in the catalog (`Z69wX` in `EfTiT`), and of the skill being
  /// edited (`ERFw4` in `ecJtg`): mono 20/500.
  VariableName,
  /// Execution context note of the instruction preview (`HvDGw`, `utiRC` in `TnjFo`): 9 muted.
  ContextNote,
  /// A table cell's label that wraps beside its icon, e.g. a worktree state (`VHSnL` in `Py65X`);
  /// a note between the scheduling review's settings (`q6NwTY`, `YPW1U` in `W1Dq6`).
  CellLabel,
  /// A field error that wraps beside its icon, e.g. why a dialog's action is unavailable at the
  /// left of its buttons (`foNvP` in 01.06 F `glurU`, `eeksu` in 02.Q states `XIEn8`).
  WrappingError,
  /// A remote MCP authentication value that wraps at 1.6 leading (`K8qeZv` in `a4lm6D`).
  WrappingProperty,
  /// A note under an inspector's details or actions: where the models come from, what the
  /// connection's status means (`xK9v9`, `Thdx2` in `JuVaA`).
  InspectorNote,
  /// Setting name in the agent enforcement panel (`vgkrV` in `CIeqc`).
  EnforcementSetting,
  /// CLI mechanism beneath an enforced setting (`x4ZFi` in `CIeqc`).
  EnforcementMechanism,
  /// Title of the plan under review, e.g. "Plan v1 · GitHub sign-in" (`J7irj` in `x3lVC`), 19/500.
  PlanTitle,
  /// A note written to an agent, e.g. the changes a plan needs (`DDXc5` in `NbNN0`): 12, 1.6.
  ReviewNote,
  /// Text an agent wrote, e.g. a message body or a plan under review (`FP6BD` in `J5MIT`): 12, 1.6.
  AgentText,
  /// An identifier as the title of a list row, e.g. a secret's name (`B1zGN` in `vxAMD`).
  MonoTitle,
  /// A stored secret shown only as its mask (`FNlaR` in `CQxCb`).
  MaskedValue,
  /// Uppercase heading of a table column, e.g. "SOURCE / VERSION" (`gHiGc` in `WcIvG`).
  ColumnHeading,
  /// `NAME=value` lines in a mono text area, e.g. the project's environment (`f0e2Hv` in `BGMgO`).
  MonoArea,
  /// A change a version makes, in the version history (`o6to5` in `DOroV`).
  ChangeLine,
  /// An acceptance criterion and its number on a task's Summary (`j0Ye4t`, `a88TT` in `tqn5P`).
  Criterion,
  /// A project's initial in a title bar tab's avatar (`T0R2Hv` in `nAKyb`): 9/600.
  TabInitial,
  /// What a message in the conversation timeline says, a person's or an agent's (`V17jfc`).
  TimelineMessage,
}

impl ExtraTypography {
  const ALL: [Self; 83] = [
    Self::SectionHeading,
    Self::CardTitle,
    Self::ReviewHeading,
    Self::Overline,
    Self::Metric,
    Self::Brand,
    Self::SetupBrand,
    Self::Display,
    Self::Lead,
    Self::Eyebrow,
    Self::Subheading,
    Self::StepNumber,
    Self::PageTitle,
    Self::Detail,
    Self::Footnote,
    Self::NoticeHeading,
    Self::ScreenTitle,
    Self::FormLabel,
    Self::StatusLabel,
    Self::NavCount,
    Self::Brief,
    Self::DialogTitle,
    Self::Explanation,
    Self::IconTag,
    Self::IconSmall,
    Self::IconReason,
    Self::IconMarker,
    Self::IconButton,
    Self::IconNav,
    Self::IconDismiss,
    Self::IconOption,
    Self::IconNotice,
    Self::IconFeature,
    Self::IconPanel,
    Self::IconWindowControl,
    Self::IconSelect,
    Self::CanvasLabel,
    Self::MonoTag,
    Self::MonoValue,
    Self::MonoLimit,
    Self::MonoField,
    Self::Instructions,
    Self::Hint,
    Self::ReadBack,
    Self::VersionFigure,
    Self::Summary,
    Self::IconLarge,
    Self::CompactStatusLabel,
    Self::InspectorTitle,
    Self::RowTitle,
    Self::IconRow,
    Self::PullRequestTitle,
    Self::PullRequestContext,
    Self::FileListHeading,
    Self::FileName,
    Self::DiffPath,
    Self::DiffLineNumber,
    Self::Code,
    Self::Output,
    Self::IconEntry,
    Self::IconEvent,
    Self::IconState,
    Self::RuleTitle,
    Self::PromptText,
    Self::VariableName,
    Self::ContextNote,
    Self::CellLabel,
    Self::WrappingError,
    Self::WrappingProperty,
    Self::InspectorNote,
    Self::EnforcementSetting,
    Self::EnforcementMechanism,
    Self::PlanTitle,
    Self::ReviewNote,
    Self::AgentText,
    Self::MonoTitle,
    Self::MaskedValue,
    Self::ColumnHeading,
    Self::MonoArea,
    Self::ChangeLine,
    Self::Criterion,
    Self::TabInitial,
    Self::TimelineMessage,
  ];

  fn name(self) -> &'static str {
    self.icon_name().unwrap_or_else(|| self.text_name())
  }

  /// The name of an icon role; `None` for a text role.
  const fn icon_name(self) -> Option<&'static str> {
    match self {
      Self::IconTag => Some("icon-tag"),
      Self::IconSmall => Some("icon-small"),
      Self::IconReason => Some("icon-reason"),
      Self::IconMarker => Some("icon-marker"),
      Self::IconButton => Some("icon-button"),
      Self::IconNav => Some("icon-nav"),
      Self::IconDismiss => Some("icon-dismiss"),
      Self::IconOption => Some("icon-option"),
      Self::IconNotice => Some("icon-notice"),
      Self::IconFeature => Some("icon-feature"),
      Self::IconPanel => Some("icon-panel"),
      Self::IconWindowControl => Some("icon-window-control"),
      Self::IconSelect => Some("icon-select"),
      Self::IconLarge => Some("icon-large"),
      Self::IconRow => Some("icon-row"),
      Self::IconEntry => Some("icon-entry"),
      Self::IconEvent => Some("icon-event"),
      Self::IconState => Some("icon-state"),
      _ => None,
    }
  }

  fn text_name(self) -> &'static str {
    match self {
      Self::SectionHeading => "section-heading",
      Self::CardTitle => "card-title",
      Self::ReviewHeading => "review-heading",
      Self::Overline => "overline",
      Self::Metric => "metric",
      Self::Brand => "brand",
      Self::SetupBrand => "setup-brand",
      Self::Display => "display",
      Self::Lead => "lead",
      Self::Eyebrow => "eyebrow",
      Self::Subheading => "subheading",
      Self::StepNumber => "step-number",
      Self::PageTitle => "page-title",
      Self::Detail => "detail",
      Self::Footnote => "footnote",
      Self::NoticeHeading => "notice-heading",
      Self::ScreenTitle => "screen-title",
      Self::FormLabel => "form-label",
      Self::StatusLabel => "status-label",
      Self::NavCount => "nav-count",
      Self::Brief => "brief",
      Self::DialogTitle => "dialog-title",
      Self::Explanation => "explanation",
      Self::CanvasLabel => "canvas-label",
      Self::MonoTag => "mono-tag",
      Self::MonoValue => "mono-value",
      Self::MonoLimit => "mono-limit",
      Self::MonoField => "mono-field",
      Self::Instructions => "instructions",
      Self::Hint => "hint",
      Self::ReadBack => "read-back",
      Self::VersionFigure => "version-figure",
      Self::Summary => "summary",
      Self::CompactStatusLabel => "compact-status-label",
      Self::InspectorTitle => "inspector-title",
      Self::RowTitle => "row-title",
      Self::PullRequestTitle => "pull-request-title",
      Self::PullRequestContext => "pull-request-context",
      Self::FileListHeading => "file-list-heading",
      Self::FileName => "file-name",
      Self::DiffPath => "diff-path",
      Self::DiffLineNumber => "diff-line-number",
      Self::Code => "code",
      Self::Output => "output",
      Self::RuleTitle => "rule-title",
      Self::PromptText => "prompt-text",
      Self::VariableName => "variable-name",
      Self::ContextNote => "context-note",
      Self::CellLabel => "cell-label",
      Self::WrappingError => "wrapping-error",
      Self::WrappingProperty => "wrapping-property",
      Self::InspectorNote => "inspector-note",
      Self::EnforcementSetting => "enforcement-setting",
      Self::EnforcementMechanism => "enforcement-mechanism",
      Self::PlanTitle => "plan-title",
      Self::ReviewNote => "review-note",
      Self::AgentText => "agent-text",
      Self::MonoTitle => "mono-title",
      Self::MaskedValue => "masked-value",
      Self::ColumnHeading => "column-heading",
      Self::MonoArea => "mono-area",
      Self::ChangeLine => "change-line",
      Self::Criterion => "criterion",
      Self::TabInitial => "tab-initial",
      Self::TimelineMessage => "timeline-message",
      icon => icon.icon_name().expect("every role without a text name is an icon"),
    }
  }

  pub fn role(self) -> TypographyStyle {
    TypographyStyle::extra(self.name())
  }
}

fn style(font_size: f32, weight: FontWeight, color: &str) -> TextStyle {
  TextStyle {
    font_family: FONT.into(),
    font_size,
    weight,
    color: Color::from_hex(color),
    ..TextStyle::default()
  }
}

/// Geist Mono without ligatures: its `liga` turns `--` into one cell-wide glyph that overlaps the
/// space before it, so commands such as `gh auth login --hostname` would read wrong.
fn mono(font_size: f32, color: &str) -> TextStyle {
  TextStyle {
    font_family: MONO.into(),
    font_features: FontFeatures::from([FontFeature::disable(*b"liga"), FontFeature::disable(*b"calt")]),
    ..style(font_size, FontWeight::Normal, color)
  }
}

/// `base` with `line_height` between wrapped lines.
fn leading(line_height: f32, base: TextStyle) -> TextStyle {
  TextStyle { line_height, ..base }
}

fn icon(font_size: f32) -> TextStyle {
  TextStyle {
    font_family: ICON_FONT.into(),
    line_height: 1.0,
    ..style(font_size, FontWeight::Normal, MUTED)
  }
}

impl ExtraTypography {
  /// The size of an icon role; `None` for a text role.
  const fn icon_size(self) -> Option<f32> {
    match self {
      Self::IconTag => Some(10.0),
      Self::IconSmall => Some(12.0),
      Self::IconReason => Some(11.0),
      Self::IconMarker => Some(13.0),
      Self::IconButton => Some(15.0),
      Self::IconNav => Some(17.0),
      Self::IconDismiss => Some(14.0),
      Self::IconOption => Some(16.0),
      Self::IconNotice => Some(18.0),
      Self::IconFeature => Some(19.0),
      Self::IconPanel => Some(24.0),
      Self::IconWindowControl => Some(14.0),
      Self::IconSelect => Some(SELECT_ICON_SIZE),
      Self::IconLarge => Some(22.0),
      Self::IconRow => Some(20.0),
      Self::IconEntry => Some(13.0),
      Self::IconEvent => Some(14.0),
      Self::IconState => Some(20.0),
      _ => None,
    }
  }

  /// The style of a Geist Mono role; `None` for the others.
  fn mono_style(self) -> Option<TextStyle> {
    Some(match self {
      Self::Metric => mono(23.0, TEXT),
      Self::StepNumber => mono(12.0, SUBTLE),
      Self::NavCount => mono(11.0, MUTED),
      Self::CanvasLabel => TextStyle {
        letter_spacing: 1.0,
        ..mono(9.0, SUBTLE)
      },
      Self::MonoTag => mono(9.0, MUTED),
      Self::MonoValue => mono(11.0, TEXT),
      Self::MonoLimit => TextStyle {
        weight: FontWeight::Medium,
        ..mono(11.0, TEXT)
      },
      Self::MonoField => mono(12.0, TEXT),
      Self::PullRequestContext => mono(11.0, MUTED),
      Self::DiffPath => TextStyle {
        weight: FontWeight::Medium,
        ..mono(14.0, TEXT)
      },
      Self::DiffLineNumber => mono(11.0, CODE_COMMENT),
      Self::Code => leading(1.6, mono(12.0, CODE_TEXT)),
      Self::Output => leading(1.6, mono(11.0, MUTED)),
      Self::PromptText => leading(1.8, mono(12.0, TEXT)),
      Self::VariableName => TextStyle {
        weight: FontWeight::Medium,
        ..mono(20.0, TEXT)
      },
      Self::MonoTitle => mono(12.0, TEXT),
      Self::MaskedValue => mono(13.0, MUTED),
      Self::MonoArea => leading(1.6, mono(12.0, TEXT)),
      _ => return None,
    })
  }

  fn style(self) -> TextStyle {
    if let Some(mono) = self.mono_style() {
      return mono;
    }
    match self {
      Self::SectionHeading => style(15.0, FontWeight::Medium, TEXT),
      Self::CardTitle => style(14.0, FontWeight::Medium, TEXT),
      Self::ReviewHeading => style(14.0, FontWeight::Normal, TEXT),
      Self::Overline => style(9.0, FontWeight::SemiBold, SUBTLE),
      Self::Brand => TextStyle {
        letter_spacing: -0.7,
        ..style(19.0, FontWeight::SemiBold, TEXT)
      },
      Self::SetupBrand => style(23.0, FontWeight::SemiBold, TEXT),
      Self::Display => TextStyle {
        letter_spacing: -1.2,
        line_height: 1.18,
        ..style(44.0, FontWeight::Medium, TEXT)
      },
      Self::Lead => leading(1.65, style(16.0, FontWeight::Normal, MUTED)),
      Self::Eyebrow => TextStyle {
        letter_spacing: 1.6,
        ..style(10.0, FontWeight::Normal, MUTED)
      },
      Self::Subheading => style(17.0, FontWeight::Medium, TEXT),
      Self::PageTitle => style(27.0, FontWeight::Medium, TEXT),
      Self::Detail => leading(1.6, style(11.0, FontWeight::Normal, MUTED)),
      Self::Footnote => style(10.0, FontWeight::Normal, MUTED),
      Self::NoticeHeading => style(12.0, FontWeight::Medium, TEXT),
      Self::ScreenTitle => style(28.0, FontWeight::Medium, TEXT),
      Self::FormLabel => style(12.0, FontWeight::Medium, TEXT),
      Self::StatusLabel => style(11.0, FontWeight::Medium, TEXT),
      Self::Brief => leading(1.7, style(13.0, FontWeight::Normal, TEXT)),
      Self::DialogTitle => style(20.0, FontWeight::Medium, TEXT),
      Self::Explanation => leading(1.6, style(12.0, FontWeight::Normal, MUTED)),
      Self::Instructions => leading(1.65, style(12.0, FontWeight::Normal, TEXT)),
      Self::Hint => leading(1.5, style(10.0, FontWeight::Normal, MUTED)),
      Self::ReadBack => style(10.0, FontWeight::Normal, SUBTLE),
      Self::VersionFigure => style(18.0, FontWeight::Medium, TEXT),
      Self::Summary => leading(1.6, style(13.0, FontWeight::Normal, MUTED)),
      Self::CompactStatusLabel => style(10.0, FontWeight::Medium, TEXT),
      Self::InspectorTitle => style(18.0, FontWeight::Medium, TEXT),
      Self::RowTitle => style(15.0, FontWeight::Medium, TEXT),
      Self::PullRequestTitle => style(24.0, FontWeight::Medium, TEXT),
      Self::FileListHeading => style(10.0, FontWeight::Medium, MUTED),
      Self::FileName => style(12.0, FontWeight::Normal, TEXT),
      Self::RuleTitle => style(20.0, FontWeight::Medium, TEXT),
      Self::ContextNote => style(9.0, FontWeight::Normal, MUTED),
      Self::CellLabel => leading(1.5, style(11.0, FontWeight::Normal, MUTED)),
      Self::WrappingError => leading(1.5, style(11.0, FontWeight::Normal, TEXT)),
      Self::WrappingProperty => leading(1.6, style(11.0, FontWeight::Normal, TEXT)),
      Self::InspectorNote => leading(1.6, style(10.0, FontWeight::Normal, MUTED)),
      Self::EnforcementSetting => leading(1.25, style(12.0, FontWeight::Normal, TEXT)),
      Self::EnforcementMechanism => leading(1.3, mono(10.0, MUTED)),
      Self::PlanTitle => style(19.0, FontWeight::Medium, TEXT),
      Self::ReviewNote => leading(1.6, style(12.0, FontWeight::Normal, TEXT)),
      Self::AgentText => leading(1.6, style(12.0, FontWeight::Normal, TEXT)),
      Self::ColumnHeading => style(9.0, FontWeight::Normal, MUTED),
      Self::ChangeLine => leading(1.4, style(12.0, FontWeight::Normal, TEXT)),
      Self::Criterion => leading(1.5, style(12.0, FontWeight::Normal, TEXT)),
      Self::TabInitial => style(9.0, FontWeight::SemiBold, TEXT),
      Self::TimelineMessage => leading(1.6, style(13.0, FontWeight::Normal, TEXT)),
      icon_role => icon(
        icon_role
          .icon_size()
          .expect("every role without a text style is an icon"),
      ),
    }
  }
}

/// Type scale read from the canonical screens (Overview `Twtqc`, Task `vpgrJ`) and
/// Foundations components.
pub fn typography() -> ThemeTypography {
  ThemeTypography {
    // Page title (`Q7Rw67`).
    heading: style(30.0, FontWeight::Medium, TEXT),
    // Section title, e.g. "Tasks" (`ypXrG`).
    title: style(16.0, FontWeight::Medium, TEXT),
    // Page description and navigation labels (`jVWaW`, `TSJpn`).
    body: style(13.0, FontWeight::Normal, TEXT),
    // Supporting copy under titles (`Hkhzg`).
    description: TextStyle {
      line_height: 1.5,
      ..style(12.0, FontWeight::Normal, MUTED)
    },
    // Metadata, counts and property rows (`wr35v`, `tfGRt`).
    caption: style(11.0, FontWeight::Normal, MUTED),
    // Panel and card headings, e.g. "Recent activity" (`q7aFf`).
    label: style(13.0, FontWeight::Medium, TEXT),
    field_label: style(12.0, FontWeight::Normal, MUTED),
    // Button label (`u1XBP`).
    button: style(12.0, FontWeight::Medium, TEXT),
    // Inline actions, e.g. "Answer question →" (`p9lUhC`).
    link: style(11.0, FontWeight::Medium, TEXT),
    // Revisions, shortcuts and counts (`q91Jf8`, `dNVoK`), in Geist Mono.
    mono: mono(10.0, MUTED),
    extra: ExtraTypography::ALL
      .into_iter()
      .map(|extra| (extra.name().into(), extra.style()))
      .collect::<HashMap<_, _>>(),
  }
}
