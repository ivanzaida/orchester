//! A proxy in front of a test server that holds the certificate, as Caddy or Traefik does for `tls = "off"`: it ends
//! TLS on loopback with a certificate from a test authority and passes the bytes on to the server's plain listener.
//! The certificate it presents can change while it runs, as at a renewal.

use std::{
  net::SocketAddr,
  sync::{Arc, LazyLock},
  time::{SystemTime, UNIX_EPOCH},
};

use parking_lot::Mutex;
use rcgen::{
  BasicConstraints, CertificateParams, CertifiedIssuer, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
  KeyUsagePurpose, date_time_ymd,
};
use tokio::{
  net::{TcpListener, TcpStream},
  task::JoinHandle,
};
use tokio_rustls::{
  TlsAcceptor,
  rustls::{
    ServerConfig,
    crypto::ring,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
  },
};

/// A certificate and its key, for a proxy to present.
pub struct Leaf {
  pub certificate: CertificateDer<'static>,
  key: PrivatePkcs8KeyDer<'static>,
}

impl Leaf {
  /// The SHA-256 (hex) a key proof names for it.
  pub fn sha256(&self) -> String {
    orchester_api::certificate::CertificatePin::of(&self.certificate).to_hex()
  }
}

/// A certificate authority made for one test process.
pub struct Authority(CertifiedIssuer<'static, KeyPair>);

/// The authority the tests' clients trust besides the operating system's store ([`trusted`]).
static TRUSTED: LazyLock<Authority> = LazyLock::new(|| Authority::generate("Orchester test authority"));

/// The trusted test authority. Every client this process makes afterwards trusts it.
pub fn trusted() -> &'static Authority {
  orchester_api::client::trust_for_tests(vec![TRUSTED.certificate()]);
  &TRUSTED
}

impl Authority {
  /// A new authority that no client trusts unless it is [`trusted`].
  pub fn generate(name: &str) -> Self {
    let mut params = CertificateParams::new(Vec::<String>::new()).expect("authority parameters");
    params.distinguished_name.push(DnType::CommonName, name);
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
      KeyUsagePurpose::KeyCertSign,
      KeyUsagePurpose::CrlSign,
      KeyUsagePurpose::DigitalSignature,
    ];
    valid_for_a_month(&mut params);
    let key = KeyPair::generate().expect("authority key");
    Self(CertifiedIssuer::self_signed(params, key).expect("authority certificate"))
  }

  pub fn certificate(&self) -> CertificateDer<'static> {
    self.0.as_ref().der().clone()
  }

  /// A new server certificate for `name`, with a key of its own.
  pub fn leaf(&self, name: &str) -> Leaf {
    let mut params = CertificateParams::new(vec![name.to_owned()]).expect("leaf parameters");
    params.distinguished_name.push(DnType::CommonName, name);
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    valid_for_a_month(&mut params);
    let key = KeyPair::generate().expect("leaf key");
    let certificate = params.signed_by(&key, &self.0).expect("leaf certificate");
    Leaf {
      certificate: certificate.der().clone(),
      key: PrivatePkcs8KeyDer::from(key.serialize_der()),
    }
  }
}

/// From yesterday for a month: short enough for any platform's limit on a server certificate's validity.
fn valid_for_a_month(params: &mut CertificateParams) {
  let days = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .expect("after 1970")
    .as_secs()
    / 86_400;
  let days = i64::try_from(days).expect("days since 1970");
  let (year, month, day) = civil(days - 1);
  params.not_before = date_time_ymd(year, month, day);
  let (year, month, day) = civil(days + 30);
  params.not_after = date_time_ymd(year, month, day);
}

/// The proleptic Gregorian date of `days` after 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil(days: i64) -> (i32, u8, u8) {
  let shifted = days + 719_468;
  let era = shifted.div_euclid(146_097);
  let of_era = shifted.rem_euclid(146_097);
  let year_of_era = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
  let day_of_year = of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
  let month_index = (5 * day_of_year + 2) / 153;
  let day = day_of_year - (153 * month_index + 2) / 5 + 1;
  let month = if month_index < 10 {
    month_index + 3
  } else {
    month_index - 9
  };
  let year = year_of_era + era * 400 + i64::from(month <= 2);
  (
    i32::try_from(year).expect("a year"),
    u8::try_from(month).expect("a month"),
    u8::try_from(day).expect("a day"),
  )
}

pub struct TlsProxy {
  presented: Arc<Mutex<Arc<ServerConfig>>>,
  task: JoinHandle<()>,
}

impl TlsProxy {
  /// Ends TLS on `listener` with `leaf` and passes each connection on to `target`.
  pub fn start(listener: TcpListener, leaf: &Leaf, target: SocketAddr) -> Self {
    let presented = Arc::new(Mutex::new(config(leaf)));
    let current = presented.clone();
    let task = tokio::spawn(async move {
      while let Ok((stream, _)) = listener.accept().await {
        let acceptor = TlsAcceptor::from(current.lock().clone());
        tokio::spawn(pass_on(acceptor, stream, target));
      }
    });
    Self { presented, task }
  }

  /// Presents `leaf` on the connections made from now on, as a proxy does after a renewal.
  pub fn present(&self, leaf: &Leaf) {
    *self.presented.lock() = config(leaf);
  }
}

impl Drop for TlsProxy {
  fn drop(&mut self) {
    self.task.abort();
  }
}

async fn pass_on(acceptor: TlsAcceptor, stream: TcpStream, target: SocketAddr) {
  // A client that refuses the certificate ends the handshake; the test sees that through its own request.
  let Ok(mut tls) = acceptor.accept(stream).await else {
    return;
  };
  let mut upstream = TcpStream::connect(target).await.expect("the test server");
  // Either side closing ends the copy; what the request got is the test's to check.
  let _ = tokio::io::copy_bidirectional(&mut tls, &mut upstream).await;
}

fn config(leaf: &Leaf) -> Arc<ServerConfig> {
  let config = ServerConfig::builder_with_provider(Arc::new(ring::default_provider()))
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .with_no_client_auth()
    .with_single_cert(
      vec![leaf.certificate.clone()],
      PrivateKeyDer::Pkcs8(leaf.key.clone_key()),
    )
    .expect("the proxy's certificate");
  Arc::new(config)
}
