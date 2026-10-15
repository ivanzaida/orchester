use std::fmt;

use ring::{
  digest::{SHA256, digest},
  rand::SystemRandom,
  signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use zeroize::Zeroize;

use super::{IdentityError, jws};

/// An Ed25519 public key, written as 32 bytes of base64url.
#[derive(Clone, PartialEq, Eq)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
  pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityError> {
    bytes.try_into().map(Self).map_err(|_| IdentityError::InvalidKey)
  }

  pub fn parse(text: &str) -> Result<Self, IdentityError> {
    Self::from_bytes(&jws::decode_part(text.trim()).map_err(|_| IdentityError::InvalidKey)?)
  }

  pub fn as_bytes(&self) -> &[u8; 32] {
    &self.0
  }

  pub fn encoded(&self) -> String {
    jws::encode(&self.0)
  }

  /// The SHA-256 of the key in base64url: what a team server pins for the cloud's root key.
  pub fn fingerprint(&self) -> String {
    jws::encode(digest(&SHA256, &self.0).as_ref())
  }

  /// A key's id is derived from the key, so a key set cannot list one id for two keys.
  pub fn kid(&self) -> String {
    jws::encode(&digest(&SHA256, &self.0).as_ref()[..16])
  }

  pub(super) fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), IdentityError> {
    UnparsedPublicKey::new(&ED25519, &self.0)
      .verify(message, signature)
      .map_err(|_| IdentityError::BadSignature)
  }
}

impl fmt::Debug for PublicKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "PublicKey({})", self.encoded())
  }
}

/// The PKCS#8 document of an Ed25519 private key: wiped on drop, never shown by `Debug`, no `Display`.
pub struct PrivateKey(Vec<u8>);

impl PrivateKey {
  pub fn generate() -> Result<Self, IdentityError> {
    Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
      .map(|document| Self(document.as_ref().to_vec()))
      .map_err(|_| IdentityError::Random)
  }

  /// Takes a stored document; it is checked when a key is made from it.
  pub fn from_pkcs8(document: Vec<u8>) -> Self {
    Self(document)
  }

  pub fn pkcs8(&self) -> &[u8] {
    &self.0
  }

  fn pair(&self) -> Result<Ed25519KeyPair, IdentityError> {
    Ed25519KeyPair::from_pkcs8(&self.0).map_err(|_| IdentityError::InvalidKey)
  }
}

impl Drop for PrivateKey {
  fn drop(&mut self) {
    self.0.zeroize();
  }
}

impl fmt::Debug for PrivateKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str("PrivateKey(<redacted>)")
  }
}

/// One of the cloud's signing keys: signs identity tokens.
pub struct SigningKey {
  kid: String,
  pair: Ed25519KeyPair,
}

impl SigningKey {
  pub fn new(private: &PrivateKey) -> Result<Self, IdentityError> {
    let pair = private.pair()?;
    let kid = public_key(&pair)?.kid();
    Ok(Self { kid, pair })
  }

  pub fn kid(&self) -> &str {
    &self.kid
  }

  pub fn public_key(&self) -> PublicKey {
    public_key(&self.pair).expect("an Ed25519 key pair has a 32-byte public key")
  }

  pub(super) fn sign(&self, message: &[u8]) -> Vec<u8> {
    self.pair.sign(message).as_ref().to_vec()
  }
}

impl fmt::Debug for SigningKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "SigningKey({})", self.kid)
  }
}

/// The cloud's root key, kept on an offline machine: signs key sets.
pub struct RootKey {
  pair: Ed25519KeyPair,
}

impl RootKey {
  pub fn new(private: &PrivateKey) -> Result<Self, IdentityError> {
    Ok(Self { pair: private.pair()? })
  }

  pub fn public_key(&self) -> PublicKey {
    public_key(&self.pair).expect("an Ed25519 key pair has a 32-byte public key")
  }

  pub(super) fn sign(&self, message: &[u8]) -> Vec<u8> {
    self.pair.sign(message).as_ref().to_vec()
  }
}

impl fmt::Debug for RootKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "RootKey({})", self.public_key().fingerprint())
  }
}

fn public_key(pair: &Ed25519KeyPair) -> Result<PublicKey, IdentityError> {
  PublicKey::from_bytes(pair.public_key().as_ref())
}
