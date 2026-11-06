//! End-to-End Encryption session cipher using X25519 and AES-256-GCM with Zeroize.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::rngs::OsRng;
use x25519_dalek::{EphemeralSecret, PublicKey};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// State holder for the active End-to-End encrypted session
pub struct SessionCipher {
    cipher: Aes256Gcm,
    send_nonce_counter: u64,
    recv_nonce_counter: u64,
}

/// Zeroize key wrapper
#[derive(Zeroize, ZeroizeOnDrop)]
struct KeyMaterial([u8; 32]);

pub struct KeyExchange {
    secret: Option<EphemeralSecret>,
    pub public_key: [u8; 32],
}

impl KeyExchange {
    pub fn new() -> Self {
        let secret = EphemeralSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        Self {
            secret: Some(secret),
            public_key: *public.as_bytes(),
        }
    }

    /// Complete key exchange with peer's public key
    pub fn complete(mut self, peer_public_bytes: &[u8; 32]) -> anyhow::Result<SessionCipher> {
        let secret = self
            .secret
            .take()
            .ok_or_else(|| anyhow::anyhow!("KeyExchange secret already used"))?;
        let peer_public = PublicKey::from(*peer_public_bytes);
        let shared_secret = secret.diffie_hellman(&peer_public);

        let mut key_bytes = KeyMaterial(*shared_secret.as_bytes());
        let cipher = Aes256Gcm::new_from_slice(&key_bytes.0)
            .map_err(|e| anyhow::anyhow!("Failed to initialize AES-GCM: {:?}", e))?;

        key_bytes.zeroize();

        Ok(SessionCipher {
            cipher,
            send_nonce_counter: 1,
            recv_nonce_counter: 1,
        })
    }
}

impl SessionCipher {
    /// Encrypt a plaintext buffer. Returns (nonce_u64, ciphertext).
    pub fn encrypt(&mut self, plaintext: &[u8]) -> anyhow::Result<(u64, Vec<u8>)> {
        let nonce_val = self.send_nonce_counter;
        self.send_nonce_counter += 1;

        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&nonce_val.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("AES-GCM encryption failed: {:?}", e))?;

        Ok((nonce_val, ciphertext))
    }

    /// Decrypt ciphertext with expected nonce (strictly monotonic to prevent replay attacks).
    pub fn decrypt(&mut self, nonce_val: u64, ciphertext: &[u8]) -> anyhow::Result<Vec<u8>> {
        // Enforce anti-replay check
        if nonce_val < self.recv_nonce_counter {
            anyhow::bail!(
                "Anti-Replay Violation: Received old nonce {} (expected >= {})",
                nonce_val,
                self.recv_nonce_counter
            );
        }
        self.recv_nonce_counter = nonce_val
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Nonce sequence counter exhausted"))?;

        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..12].copy_from_slice(&nonce_val.to_be_bytes());
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("AES-GCM decryption failed: {:?}", e))?;

        Ok(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_e2e_key_exchange_and_cipher() {
        let kx_host = KeyExchange::new();
        let kx_viewer = KeyExchange::new();

        let host_pub = kx_host.public_key;
        let viewer_pub = kx_viewer.public_key;

        let mut host_cipher = kx_host.complete(&viewer_pub).unwrap();
        let mut viewer_cipher = kx_viewer.complete(&host_pub).unwrap();

        let secret_msg = b"FreeDesk 60FPS High-Security Stream Payload";
        let (nonce, ciphertext) = host_cipher.encrypt(secret_msg).unwrap();

        let decrypted = viewer_cipher.decrypt(nonce, &ciphertext).unwrap();
        assert_eq!(decrypted, secret_msg);

        // Security Test 1: Replay attack (re-sending same nonce) must fail
        assert!(viewer_cipher.decrypt(nonce, &ciphertext).is_err());

        // Security Test 2: Old nonce must fail
        assert!(viewer_cipher.decrypt(nonce - 1, &ciphertext).is_err());
    }
}
