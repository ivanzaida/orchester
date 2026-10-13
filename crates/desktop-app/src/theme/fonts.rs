use lurq::app::{
  App,
  theme::{Theme, TypographyStyle},
};

/// UI family (`font_sans`): Geist v1.7.2 from `vercel/geist-font`, OFL (`assets/fonts/Geist-OFL.txt`).
pub const SANS: &str = "Geist";
/// Code, paths and keys (`font_mono`): Geist Mono, same source and license.
pub const MONO: &str = "Geist Mono";
/// How far every glyph of Geist Mono advances, in em: 600 of its 1000 units, a space and a bullet alike.
const MONO_ADVANCE: f32 = 0.6;

/// How wide a space is in `style`, a role in Geist Mono.
pub fn mono_space(theme: &Theme, style: impl Into<TypographyStyle>) -> f32 {
  theme.typography().get(style).font_size * MONO_ADVANCE
}
/// Family name inside the bundled `lucide.ttf`, ISC (`assets/fonts/lucide-LICENSE.txt`).
pub const ICONS: &str = "lucide";

/// The faces the type roles use: Geist at 400, 500 and 600, Geist Mono at 400, and the icons.
const FACES: [&[u8]; 5] = [
  include_bytes!("../../assets/fonts/Geist-Regular.ttf"),
  include_bytes!("../../assets/fonts/Geist-Medium.ttf"),
  include_bytes!("../../assets/fonts/Geist-SemiBold.ttf"),
  include_bytes!("../../assets/fonts/GeistMono-Regular.ttf"),
  include_bytes!("../../assets/fonts/lucide.ttf"),
];

pub fn install(app: &mut App) {
  app.install_fonts(FACES.into_iter().map(<[u8]>::to_vec), [] as [(&str, &str); 0]);
}
