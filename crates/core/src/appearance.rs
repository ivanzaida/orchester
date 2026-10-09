//! How a person wants the app's window colored, kept with the application's settings.

/// Dark, light, or whatever the operating system's appearance is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeSetting {
  #[default]
  Dark,
  Light,
  System,
}

impl ThemeSetting {
  pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::System];

  /// The name it is stored and requested under.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Dark => "dark",
      Self::Light => "light",
      Self::System => "system",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|setting| setting.name() == name)
  }
}
