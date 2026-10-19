use std::num::NonZeroU32;

use ring::{
    aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
    pbkdf2,
    rand::{SecureRandom, SystemRandom},
};

const VERSION: u8 = 1;
const NONCE_LEN: usize = 12;

pub struct Cipher(LessSafeKey);

impl Cipher {
    #[cfg(test)]
    pub fn new(passphrase: &str, room: &str) -> Result<Self, String> {
        Self::from_key(&Self::derive_key(passphrase, room)?)
    }

    pub fn from_key(key: &[u8; 32]) -> Result<Self, String> {
        let key = UnboundKey::new(&aead::CHACHA20_POLY1305, key)
            .map_err(|_| "could not initialize media encryption")?;
        Ok(Self(LessSafeKey::new(key)))
    }

    pub fn derive_key(passphrase: &str, room: &str) -> Result<[u8; 32], String> {
        if passphrase.len() < 16 || room.is_empty() {
            return Err(
                "PULSE_E2E_PASSPHRASE must have at least 16 bytes and PULSE_ROOM must be set"
                    .into(),
            );
        }
        let mut key = [0u8; 32];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(210_000).unwrap(),
            format!("pulse-room-e2e-v1:{room}").as_bytes(),
            passphrase.as_bytes(),
            &mut key,
        );
        Ok(key)
    }

    pub fn seal(&self, publisher: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; NONCE_LEN];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| "could not generate encryption nonce")?;
        let mut encrypted = plaintext.to_vec();
        self.0
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(publisher.as_bytes()),
                &mut encrypted,
            )
            .map_err(|_| "could not encrypt media")?;
        let mut packet = Vec::with_capacity(1 + NONCE_LEN + encrypted.len());
        packet.push(VERSION);
        packet.extend_from_slice(&nonce);
        packet.extend_from_slice(&encrypted);
        Ok(packet)
    }

    pub fn open(&self, publisher: &str, packet: &[u8]) -> Result<Vec<u8>, String> {
        if packet.len() < 1 + NONCE_LEN + aead::CHACHA20_POLY1305.tag_len() || packet[0] != VERSION
        {
            return Err("invalid encrypted media packet".into());
        }
        let nonce: [u8; NONCE_LEN] = packet[1..1 + NONCE_LEN].try_into().unwrap();
        let mut encrypted = packet[1 + NONCE_LEN..].to_vec();
        let plaintext = self
            .0
            .open_in_place(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(publisher.as_bytes()),
                &mut encrypted,
            )
            .map_err(|_| "media authentication failed")?;
        Ok(plaintext.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::Cipher;

    #[test]
    fn room_key_authenticates_publisher_and_payload() {
        let alice = Cipher::new("a long shared passphrase", "friends").unwrap();
        let bob = Cipher::new("a long shared passphrase", "friends").unwrap();
        let packet = alice.seal("alice", b"opus frame").unwrap();
        assert_eq!(bob.open("alice", &packet).unwrap(), b"opus frame");
        assert!(bob.open("mallory", &packet).is_err());
        assert!(Cipher::new("a long shared passphrase", "other")
            .unwrap()
            .open("alice", &packet)
            .is_err());
        let mut tampered = packet;
        *tampered.last_mut().unwrap() ^= 1;
        assert!(bob.open("alice", &tampered).is_err());
    }

    #[test]
    fn server_issued_room_key_encrypts_for_all_members() {
        let key = [7u8; 32];
        let alice = Cipher::from_key(&key).unwrap();
        let bob = Cipher::from_key(&key).unwrap();
        let packet = alice.seal("alice", b"shared audio").unwrap();
        assert_eq!(bob.open("alice", &packet).unwrap(), b"shared audio");
        assert!(Cipher::from_key(&[8u8; 32])
            .unwrap()
            .open("alice", &packet)
            .is_err());
    }
}
