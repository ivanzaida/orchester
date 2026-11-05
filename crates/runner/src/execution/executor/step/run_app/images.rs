//! Converts PNG blocks to run-owned files in the run's own storage, `.orchester/runs/<run>/screenshots/` (ORC-114):
//! the task page lists them under Artifacts, and cleaning or reusing a workspace never removes them. Re-encoding
//! discards metadata that could contain credentials. The agent opens a saved file with `read_file`, which shows it on
//! Claude Code.

use std::{
  fs,
  io::Cursor,
  path::{Path, PathBuf},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, ImageReader};
use orchester_agent::files::image::decoding_limits;
use serde_json::{Value, json};

use super::AppError;

/// The folder of the run's storage the screenshots go to.
const SCREENSHOTS: &str = "screenshots";

pub(super) fn save(artifacts: &Path, result: &mut Value) -> Result<(), AppError> {
  let content = result["content"].as_array_mut().ok_or(AppError::Content)?;
  for block in content {
    match block["type"].as_str() {
      Some("text") if block["text"].is_string() => {}
      Some("image") => {
        let saved = save_png(artifacts, block)?;
        *block = json!({"type":"text","text":format!("PNG saved: {saved}; open it with read_file to see it")});
      }
      _ => return Err(AppError::Content),
    }
  }
  Ok(())
}

/// Saves the PNG of `block` in the screenshots of the run's storage `artifacts`; its absolute path, with `/`.
fn save_png(artifacts: &Path, block: &Value) -> Result<String, AppError> {
  if block["mimeType"] != "image/png" {
    return Err(AppError::Image);
  }
  let encoded = block["data"].as_str().ok_or(AppError::Image)?;
  if encoded.len() > 64 * 1024 * 1024 {
    return Err(AppError::Image);
  }
  let bytes = STANDARD.decode(encoded).map_err(|_| AppError::Image)?;
  let mut reader = ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
  reader.limits(decoding_limits());
  let image = reader.decode().map_err(|_| AppError::Image)?;
  let folder = output_folder(artifacts)?;
  let mut file = tempfile::Builder::new()
    .prefix("app-")
    .suffix(".png")
    .tempfile_in(folder)
    .map_err(|_| AppError::SaveImage)?;
  image
    .write_to(&mut file, ImageFormat::Png)
    .map_err(|_| AppError::SaveImage)?;
  let (_, path) = file.keep().map_err(|_| AppError::SaveImage)?;
  Ok(path.to_string_lossy().replace('\\', "/"))
}

/// The screenshots folder of the run's storage, made when it is not there, as spelled from `artifacts` (Windows would
/// name a resolved one in its verbatim `\\?\` form, which the file tools read no path in); refused when a link put
/// there leads out of the storage.
fn output_folder(artifacts: &Path) -> Result<PathBuf, AppError> {
  let folder = artifacts.join(SCREENSHOTS);
  fs::create_dir_all(&folder).map_err(|_| AppError::SaveImage)?;
  let storage = fs::canonicalize(artifacts).map_err(|_| AppError::SaveImage)?;
  let resolved = fs::canonicalize(&folder).map_err(|_| AppError::SaveImage)?;
  if !resolved.starts_with(&storage) || !resolved.is_dir() {
    return Err(AppError::SaveImage);
  }
  Ok(folder)
}
