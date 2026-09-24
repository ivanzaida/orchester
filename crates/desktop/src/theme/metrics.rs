use lurq::{
  app::theme::{RadiusSize, SpacingSize, ThemeBorderSizes, ThemeRadii, ThemeSpacing},
  node::dimension::Dimension,
};

use super::tokens::{
  FOCUS_STROKE, RADIUS_CONTROL, RADIUS_ITEM, RADIUS_MARK, RADIUS_NOTICE, RADIUS_PANEL, SPACE_COLUMNS, SPACE_DIVIDER,
  SPACE_FIELD, SPACE_FOOTER, SPACE_FORM, SPACE_GROUP, SPACE_HEADING_BOTTOM, SPACE_HEADING_TOP, SPACE_HERO,
  SPACE_INDENT, SPACE_INDICATOR, SPACE_INLINE, SPACE_LG, SPACE_MARGIN, SPACE_MARK, SPACE_MD, SPACE_PAGE,
  SPACE_PROPERTY, SPACE_RAIL, SPACE_SECTION, SPACE_SM, SPACE_STACK, SPACE_STEP, SPACE_TIGHT, TAB_INDICATOR_STROKE,
};

/// Pencil gaps and paddings without a built-in lurq role, registered as `SpacingSize::Extra`.
/// `SpacingSize::Section` is Pencil's 32 (topbar and overview padding, `CsB3h`, `FMRs3`).
#[derive(Clone, Copy)]
pub enum ExtraSpacing {
  /// Gap between the bars of the Orchester mark (`DIMZ6`).
  Mark,
  /// Notice heading to its detail (`w8Szl`).
  Tight,
  /// Status indicator to its label, task title to its metadata (`rvPqs`, `i3svLs`).
  Indicator,
  /// Icon to label in navigation items and the brand (`TSJpn`, `a0S44m`).
  Inline,
  /// Title to description, notice content, nav item padding (`b0m9CN`, `q5LzI`, `TSJpn`).
  Stack,
  /// Loosely grouped items: primary action and caption, footer items (`s9JMb`, `tNThR`).
  Group,
  /// Getting started steps (`ZZqwk`, `XVkaB` instances).
  Step,
  /// Setup rail padding and page heading top padding (`Hw5eX`, `b0m9CN`).
  Rail,
  /// Top padding of a project screen heading (`LIfyO`).
  HeadingTop,
  /// Bottom padding of a project screen heading; brief to its hint (`LIfyO`, `bGakk`).
  HeadingBottom,
  /// Gap between the groups of a configuration form (`xT2ex`, `dCLMl`).
  Form,
  /// Left indent of the getting started steps after their divider; horizontal padding and
  /// column gap of the new task form (`ZZqwk`, `hkWbp`, `uZeUx`).
  Indent,
  /// Horizontal page padding of the welcome topbar and the setup form (`DY5WC`, `Allwo`).
  Page,
  /// Gap between the welcome content and its footer (`Wl8bU`).
  Hero,
  /// Bottom padding of the welcome content (`Wl8bU`).
  Footer,
  /// Gap between the welcome columns and top padding of the welcome content (`j7w1Cj`, `Wl8bU`).
  Columns,
  /// Horizontal margin of the welcome content (`Wl8bU`).
  Margin,
  /// Field label to its control in the pipeline inspector and dialogs (`x9u2n`, `J4kw6q`).
  Field,
  /// Between key/value properties of the step inspector (`dSLqh`).
  Property,
  /// Above the properties after their divider (`dSLqh`).
  Divider,
}

impl ExtraSpacing {
  const ALL: [Self; 20] = [
    Self::Mark,
    Self::Tight,
    Self::Indicator,
    Self::Inline,
    Self::Stack,
    Self::Group,
    Self::Step,
    Self::Rail,
    Self::HeadingTop,
    Self::HeadingBottom,
    Self::Form,
    Self::Indent,
    Self::Page,
    Self::Hero,
    Self::Footer,
    Self::Columns,
    Self::Margin,
    Self::Field,
    Self::Property,
    Self::Divider,
  ];

  fn name(self) -> &'static str {
    match self {
      Self::Mark => "mark",
      Self::Tight => "tight",
      Self::Indicator => "indicator",
      Self::Inline => "inline",
      Self::Stack => "stack",
      Self::Group => "group",
      Self::Step => "step",
      Self::Rail => "rail",
      Self::HeadingTop => "heading-top",
      Self::HeadingBottom => "heading-bottom",
      Self::Form => "form",
      Self::Indent => "indent",
      Self::Page => "page",
      Self::Hero => "hero",
      Self::Footer => "footer",
      Self::Columns => "columns",
      Self::Margin => "margin",
      Self::Field => "field",
      Self::Property => "property",
      Self::Divider => "divider",
    }
  }

  fn value(self) -> f32 {
    match self {
      Self::Mark => SPACE_MARK,
      Self::Tight => SPACE_TIGHT,
      Self::Indicator => SPACE_INDICATOR,
      Self::Inline => SPACE_INLINE,
      Self::Stack => SPACE_STACK,
      Self::Group => SPACE_GROUP,
      Self::Step => SPACE_STEP,
      Self::Rail => SPACE_RAIL,
      Self::HeadingTop => SPACE_HEADING_TOP,
      Self::HeadingBottom => SPACE_HEADING_BOTTOM,
      Self::Form => SPACE_FORM,
      Self::Indent => SPACE_INDENT,
      Self::Page => SPACE_PAGE,
      Self::Hero => SPACE_HERO,
      Self::Footer => SPACE_FOOTER,
      Self::Columns => SPACE_COLUMNS,
      Self::Margin => SPACE_MARGIN,
      Self::Field => SPACE_FIELD,
      Self::Property => SPACE_PROPERTY,
      Self::Divider => SPACE_DIVIDER,
    }
  }

  pub fn role(self) -> SpacingSize {
    SpacingSize::extra(self.name())
  }
}

/// Pencil radii without a built-in lurq role, registered as `RadiusSize::Extra`.
#[derive(Clone, Copy)]
pub enum ExtraRadius {
  /// Inline notices, the runner status card, the task state tile and the brief field
  /// (`q5LzI`, `O2Jjjf`, `aqRNL`, `bGakk`).
  Notice,
  /// Bars of the Orchester mark (`DIMZ6`).
  Mark,
}

impl ExtraRadius {
  const ALL: [Self; 2] = [Self::Notice, Self::Mark];

  fn name(self) -> &'static str {
    match self {
      Self::Notice => "notice",
      Self::Mark => "mark",
    }
  }

  fn value(self) -> f32 {
    match self {
      Self::Notice => RADIUS_NOTICE,
      Self::Mark => RADIUS_MARK,
    }
  }

  pub fn role(self) -> RadiusSize {
    RadiusSize::extra(self.name())
  }
}

pub fn radii() -> ThemeRadii {
  ThemeRadii {
    sm: RADIUS_ITEM,
    md: RADIUS_CONTROL,
    lg: RADIUS_PANEL,
    extra: ExtraRadius::ALL
      .into_iter()
      .map(|radius| (radius.name().into(), radius.value()))
      .collect(),
  }
}

/// Pencil defines `space-sm/md/lg` only; `xs` and `xl` keep lurq defaults.
pub fn spacing() -> ThemeSpacing {
  ThemeSpacing {
    sm: Dimension::Px(SPACE_SM),
    md: Dimension::Px(SPACE_MD),
    lg: Dimension::Px(SPACE_LG),
    section: Dimension::Px(SPACE_SECTION),
    extra: ExtraSpacing::ALL
      .into_iter()
      .map(|spacing| (spacing.name().into(), Dimension::Px(spacing.value())))
      .collect(),
    ..ThemeSpacing::default()
  }
}

pub fn border_sizes() -> ThemeBorderSizes {
  ThemeBorderSizes {
    md: FOCUS_STROKE,
    lg: TAB_INDICATOR_STROKE,
    ..ThemeBorderSizes::default()
  }
}
