//! The certificate the server presents (`docs/architecture/server.md`, "Transport and certificates"): its own,
//! generated on the first start and kept in `<data_dir>/tls/`, or the team's PEM files.

use std::{
  io::Write,
  path::{Path, PathBuf},
};

use orchester_api::certificate::CertificatePin;
use rcgen::{CertificateParams, DnType, KeyPair, PKCS_ECDSA_P256_SHA256, date_time_ymd};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

use crate::{
  config::{Config, TlsMode},
  error::ServerError,
};

const VALID_YEARS: i32 = 20;

/// A certificate chain with its key, and the pin of its leaf.
pub(crate) struct Material {
  pub chain: Vec<CertificateDer<'static>>,
  pub key: PrivateKeyDer<'static>,
  pub pin: CertificatePin,
}

/// The certificate `config` asks for: the generated one (made now if the data directory has none), the team's PEM
/// files, or none for plain HTTP.
pub(crate) fn load(config: &Config, now_ms: i64) -> Result<Option<Material>, ServerError> {
  match config.tls {
    TlsMode::Off => Ok(None),
    TlsMode::Pem => {
      let (Some(cert), Some(key)) = (&config.tls_cert, &config.tls_key) else {
        return Err(ServerError::Configuration(
          "tls = \"pem\" needs both TLS_CERT and TLS_KEY",
        ));
      };
      read(cert, key).map(Some)
    }
    TlsMode::SelfSigned => {
      let (cert, key) = paths(&config.data_dir);
      match (cert.exists(), key.exists()) {
        (true, true) => read(&cert, &key).map(Some),
        (false, false) => generate(config, now_ms).map(Some),
        _ => Err(ServerError::Configuration(
          "the tls folder holds only half of the certificate; run `orchester-server tls regenerate`",
        )),
      }
    }
  }
}

/// Replaces the generated certificate (`orchester-server tls regenerate`): every pin of the old one breaks on purpose.
pub(crate) fn regenerate(config: &Config, now_ms: i64) -> Result<Material, ServerError> {
  if config.tls != TlsMode::SelfSigned {
    return Err(ServerError::Configuration(
      "only tls = \"self_signed\" generates its certificate",
    ));
  }
  let (cert, key) = paths(&config.data_dir);
  for path in [&cert, &key] {
    match std::fs::remove_file(path) {
      Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
      _ => {}
    }
  }
  generate(config, now_ms)
}

fn paths(data_dir: &Path) -> (PathBuf, PathBuf) {
  let folder = data_dir.join("tls");
  (folder.join("cert.pem"), folder.join("key.pem"))
}

fn read(cert: &Path, key: &Path) -> Result<Material, ServerError> {
  let chain = CertificateDer::pem_file_iter(cert)
    .map_err(|_| ServerError::Tls)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|_| ServerError::Tls)?;
  let key = PrivateKeyDer::from_pem_file(key).map_err(|_| ServerError::Tls)?;
  let pin = CertificatePin::of(chain.first().ok_or(ServerError::Tls)?);
  Ok(Material { chain, key, pin })
}

/// An ECDSA P-256 key and a self-signed certificate valid for 20 years from today.
fn generate(config: &Config, now_ms: i64) -> Result<Material, ServerError> {
  let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|_| ServerError::Tls)?;
  let mut params = CertificateParams::new(names(config)).map_err(|_| ServerError::Tls)?;
  params.distinguished_name.push(DnType::CommonName, config.name.clone());
  let (year, month, day) = civil_date(now_ms);
  params.not_before = date_time_ymd(year, month, day);
  params.not_after = date_time_ymd(year + VALID_YEARS, month, day.min(28));
  let certificate = params.self_signed(&key).map_err(|_| ServerError::Tls)?;
  let (cert_path, key_path) = paths(&config.data_dir);
  std::fs::create_dir_all(cert_path.parent().expect("the tls folder has a parent"))?;
  orchester_core::private_file::create(&key_path)?.write_all(key.serialize_pem().as_bytes())?;
  std::fs::write(&cert_path, certificate.pem())?;
  read(&cert_path, &key_path)
}

/// `localhost`, the loopback addresses, the address the server listens on when it names one, the configured
/// `tls_names`, and the computer's name when the environment gives it. Names do not matter to a pinned desktop.
fn names(config: &Config) -> Vec<String> {
  let mut names = vec!["localhost".to_owned(), "127.0.0.1".to_owned(), "::1".to_owned()];
  let listen = config.listen.ip();
  if !listen.is_unspecified() && !listen.is_loopback() {
    names.push(listen.to_string());
  }
  let host = ["HOSTNAME", "COMPUTERNAME"]
    .into_iter()
    .find_map(|key| std::env::var(key).ok())
    .map(|name| name.to_ascii_lowercase())
    .filter(|name| {
      !name.is_empty()
        && name
          .bytes()
          .all(|byte| byte.is_ascii_alphanumeric() || b"-.".contains(&byte))
    });
  for name in config.tls_names.iter().cloned().chain(host) {
    if !names.contains(&name) {
      names.push(name);
    }
  }
  names
}

/// The UTC calendar date of a Unix time in milliseconds (Howard Hinnant's `civil_from_days`).
fn civil_date(now_ms: i64) -> (i32, u8, u8) {
  let days = now_ms.div_euclid(86_400_000) + 719_468;
  let era = days.div_euclid(146_097);
  let day_of_era = days.rem_euclid(146_097);
  let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
  let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let shifted_month = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
  let month = if shifted_month < 10 {
    shifted_month + 3
  } else {
    shifted_month - 9
  };
  let year = year_of_era + era * 400 + i64::from(month <= 2);
  (
    i32::try_from(year).expect("server time is within a few thousand years"),
    u8::try_from(month).expect("a month is 1 to 12"),
    u8::try_from(day).expect("a day is 1 to 31"),
  )
}
