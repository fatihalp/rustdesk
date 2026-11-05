use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use bytes::Bytes;
use ed25519_dalek::{Signer, SigningKey};
use protobuf::Message;

use crate::proto::id_pk::IdPk;

const PRIVATE_KEY_FILE: &str = "id_ed25519";
const PUBLIC_KEY_FILE: &str = "id_ed25519.pub";

pub struct ServerKeys {
    signing: SigningKey,
    public_key: String,
}

impl ServerKeys {
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        let private_path = dir.join(PRIVATE_KEY_FILE);
        let signing = if private_path.exists() {
            read_private_key(&private_path)?
        } else {
            let signing = generate()?;
            write_private_key(&private_path, &signing)?;
            signing
        };
        let public_key = STANDARD.encode(signing.verifying_key().as_bytes());
        fs::write(dir.join(PUBLIC_KEY_FILE), &public_key).context("failed to write public key")?;
        Ok(Self { signing, public_key })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn sign_id_pk(&self, id: &str, pk: &[u8]) -> Bytes {
        let payload = IdPk {
            id: id.to_owned(),
            pk: Bytes::copy_from_slice(pk),
            ..Default::default()
        }
        .write_to_bytes()
        .unwrap_or_default();
        let mut signed = self.signing.sign(&payload).to_bytes().to_vec();
        signed.extend_from_slice(&payload);
        signed.into()
    }
}

fn generate() -> Result<SigningKey> {
    for _ in 0..300 {
        let signing = SigningKey::from_bytes(&rand::random());
        if !STANDARD.encode(signing.verifying_key().as_bytes()).contains('/') {
            return Ok(signing);
        }
    }
    anyhow::bail!("failed to generate a suitable key pair")
}

fn read_private_key(path: &Path) -> Result<SigningKey> {
    let encoded = fs::read_to_string(path).context("failed to read private key")?;
    let bytes = STANDARD.decode(encoded.trim()).context("malformed private key")?;
    let keypair: [u8; 64] = bytes.try_into().map_err(|_| anyhow::anyhow!("malformed private key"))?;
    SigningKey::from_keypair_bytes(&keypair).context("malformed private key")
}

fn write_private_key(path: &Path, signing: &SigningKey) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .context("failed to create private key file")?;
    file.write_all(STANDARD.encode(signing.to_keypair_bytes()).as_bytes())
        .context("failed to write private key")
}
