use std::fmt::{Debug, Formatter};

use anyhow::Result;
use hkdf::Hkdf;
use rand::{rngs::OsRng, CryptoRng, RngCore};
use sha2::{Digest, Sha256};
use x25519_dalek::{EphemeralSecret, PublicKey};

pub struct SessionKey {
    sym_key: [u8; 32],
    public_key: PublicKey,
}

impl Debug for SessionKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WalletConnectUrl")
            .field("sym_key", &"********")
            .field("public_key", &self.public_key)
            .finish()
    }
}

impl SessionKey {
    pub fn from_osrng(sender_public_key: &[u8; 32]) -> Result<Self> {
        SessionKey::diffie_hellman(OsRng, sender_public_key)
    }

    pub fn diffie_hellman<T>(csprng: T, sender_public_key: &[u8; 32]) -> Result<Self>
    where
        T: RngCore + CryptoRng,
    {
        let single_use_private_key = EphemeralSecret::random_from_rng(csprng);
        let public_key = PublicKey::from(&single_use_private_key);

        let ikm = single_use_private_key.diffie_hellman(&PublicKey::from(*sender_public_key));

        let mut session_sym_key = Self {
            sym_key: [0u8; 32],
            public_key,
        };
        let hk = Hkdf::<Sha256>::new(None, ikm.as_bytes());
        hk.expand(&[], &mut session_sym_key.sym_key)
            .map_err(|e| anyhow::anyhow!("Failed to generate SymKey: {e}"))?;

        Ok(session_sym_key)
    }

    pub fn symmetric_key(&self) -> &[u8; 32] {
        &self.sym_key
    }

    pub fn diffie_public_key(&self) -> &[u8; 32] {
        self.public_key.as_bytes()
    }

    pub fn generate_topic(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.sym_key);
        hex::encode(hasher.finalize())
    }
}
