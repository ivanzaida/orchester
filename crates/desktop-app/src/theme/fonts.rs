use lurq::app::App;

/// UI family (`font_sans`): Geist v1.7.2 from `vercel/geist-font`, OFL (`assets/fonts/Geist-OFL.txt`).
pub const SANS: &str = "Geist";
/// Code, paths and keys (`font_mono`): Geist Mono, same source and license.
pub const MONO: &str = "Geist Mono";
/// Family name inside the bundled `lucide.ttf`, ISC (`assets/fonts/lucide-LICENSE.txt`).
pub const ICONS: &str = "lucide";

const FACES: [&[u8]; 6] = [
  include_bytes!("../../assets/fonts/Geist-Regular.ttf"),
  include_bytes!("../../assets/fonts/Geist-Medium.ttf"),
  include_bytes!("../../assets/fonts/Geist-SemiBold.ttf"),
  include_bytes!("../../assets/fonts/Geist-Bold.ttf"),
  include_bytes!("../../assets/fonts/GeistMono-Regular.ttf"),
  include_bytes!("../../assets/fonts/lucide.ttf"),
];

pub fn install(app: &mut App) {
  app.install_fonts(FACES.into_iter().map(<[u8]>::to_vec), [] as [(&str, &str); 0]);
}
