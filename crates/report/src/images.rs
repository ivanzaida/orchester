use sentry::protocol::{AppleDebugImage, DebugImage, DebugMeta, SymbolicDebugImage};

use crate::event::ReportScrubber;

impl ReportScrubber {
  /// The loaded program files GlitchTip matches a report's frame addresses to its uploaded debug files by: each
  /// image's debug and code ids, where it was loaded and its size, with the file names of the program and of its
  /// debug file (PDB) but not their folders. Other kinds of image and the system SDK's details are dropped.
  pub(crate) fn debug_meta(&self, meta: DebugMeta) -> DebugMeta {
    DebugMeta {
      images: meta.images.into_iter().filter_map(|image| self.image(image)).collect(),
      ..DebugMeta::default()
    }
  }

  fn image(&self, image: DebugImage) -> Option<DebugImage> {
    match image {
      DebugImage::Symbolic(image) => Some(
        SymbolicDebugImage {
          name: self.file_name(&image.name),
          debug_file: image.debug_file.map(|file| self.file_name(&file)),
          ..image
        }
        .into(),
      ),
      DebugImage::Apple(image) => Some(
        AppleDebugImage {
          name: self.file_name(&image.name),
          ..image
        }
        .into(),
      ),
      DebugImage::Proguard(_) | DebugImage::Wasm(_) => None,
    }
  }

  /// The last name of `path`, written with `\` or `/`, as the scrubbed texts read it.
  fn file_name(&self, path: &str) -> String {
    let name = path.rfind(['/', '\\']).map_or(path, |separator| &path[separator + 1..]);
    self.text(name)
  }
}
