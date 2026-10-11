//! Converts PNG blocks to run-owned files. Re-encoding discards metadata that could contain credentials.

use std::{
  fs,
  io::Cursor,
  path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader, Limits};
use serde_json::{Value, json};

use super::AppError;

pub(super) fn save(worktree: &Path, result: &mut Value) -> Result<(), AppError> {
  let content = result["content"].as_array_mut().ok_or(AppError::Content)?;
  for block in content {
    match block["type"].as_str() {
      Some("text") if block["text"].is_string() => {}
      Some("image") => {
        let relative = save_png(worktree, block)?;
        *block = json!({"type":"text","text":format!("PNG saved: {relative}")});
      }
      _ => return Err(AppError::Content),
    }
  }
  Ok(())
}

fn save_png(worktree: &Path, block: &Value) -> Result<String, AppError> {
  if block["mimeType"] != "image/png" {
    return Err(AppError::Image);
  }
  let encoded = block["data"].as_str().ok_or(AppError::Image)?;
  if encoded.len() > 64 * 1024 * 1024 {
    return Err(AppError::Image);
  }
  let bytes = STANDARD.decode(encoded).map_err(|_| AppError::Image)?;
  let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
  let mut limits = Limits::default();
  limits.max_alloc = Some(128 * 1024 * 1024);
  limits.max_image_width = Some(8192);
  limits.max_image_height = Some(8192);
  reader.limits(limits);
  let image = reader.decode().map_err(|_| AppError::Image)?;
  let root = fs::canonicalize(worktree).map_err(|_| AppError::SaveImage)?;
  let folder = output_folder(&root)?;
  let mut file = tempfile::Builder::new()
    .prefix("app-")
    .suffix(".png")
    .tempfile_in(folder)
    .map_err(|_| AppError::SaveImage)?;
  image
    .write_to(&mut file, ImageFormat::Png)
    .map_err(|_| AppError::SaveImage)?;
  let (_, path) = file.keep().map_err(|_| AppError::SaveImage)?;
  path
    .strip_prefix(&root)
    .map(|path| path.to_string_lossy().replace('\\', "/"))
    .map_err(|_| AppError::SaveImage)
}

fn output_folder(root: &Path) -> Result<PathBuf, AppError> {
  let mut folder = root.to_path_buf();
  for name in [".tmp", "run-app"] {
    let path = folder.join(name);
    match fs::create_dir(&path) {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
      Err(_) => return Err(AppError::SaveImage),
    }
    folder = fs::canonicalize(path).map_err(|_| AppError::SaveImage)?;
    if !folder.starts_with(root) || !folder.is_dir() {
      return Err(AppError::SaveImage);
    }
  }
  Ok(folder)
}
