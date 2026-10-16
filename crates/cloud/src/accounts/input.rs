use orchester_api::cloud::{CloudErrorCode, DeviceInfo};

use crate::error::CloudError;

/// An email as the cloud keeps it: trimmed and lowercased, one `@`, a dotted domain, no spaces or control characters.
/// Deliverability is proven by the confirmation link, not by parsing.
pub(super) fn email(value: &str) -> Result<String, CloudError> {
  let email = value.trim().to_lowercase();
  let Some((local, domain)) = email.split_once('@') else {
    return Err(CloudErrorCode::Invalid.into());
  };
  let plain = |part: &str| {
    !part.is_empty()
      && !part
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || "@<>,;:\"()[]\\".contains(c))
  };
  let dotted = domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !domain.contains("..");
  if email.len() > 254 || local.len() > 64 || !plain(local) || !plain(domain) || !dotted {
    return Err(CloudErrorCode::Invalid.into());
  }
  Ok(email)
}

pub(super) fn display_name(value: &str) -> Result<String, CloudError> {
  text(value, 100)
}

pub(super) fn device(device: &DeviceInfo) -> Result<DeviceInfo, CloudError> {
  Ok(DeviceInfo {
    name: text(&device.name, 256)?,
    platform: device.platform,
  })
}

fn text(value: &str, max: usize) -> Result<String, CloudError> {
  let value = value.trim();
  let length = value.chars().count();
  if length == 0 || length > max || value.chars().any(char::is_control) {
    return Err(CloudErrorCode::Invalid.into());
  }
  Ok(value.to_owned())
}
