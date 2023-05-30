//! This file provides ability of encryption/decryption.
use crate::prelude::*;
use cfg_if::cfg_if;
#[cfg(not(feature = "sgx"))]
use openssl::rand::rand_bytes;
#[cfg(not(feature = "sgx"))]
use openssl::symm::{decrypt, decrypt_aead, encrypt, encrypt_aead, Cipher};
#[cfg(feature = "sgx")]
use sgx_rand::{thread_rng, Rng};
#[cfg(feature = "sgx")]
use sgx_tcrypto::{rsgx_aes_ctr_decrypt, rsgx_aes_ctr_encrypt};
#[cfg(feature = "sgx")]
use sgx_tcrypto::{rsgx_rijndael128GCM_decrypt, rsgx_rijndael128GCM_encrypt};
#[cfg(feature = "sgx")]
use sgx_types::sgx_status_t;

pub const CIPHER_SIZE: usize = BLOCK_SIZE;
pub const AUTH_ENC_KEY_SIZE: usize = 16;
pub const AUTH_ENC_MAC_SIZE: usize = 16;
pub const AUTH_ENC_IV_SIZE: usize = 12;

pub type Key = [u8; AUTH_ENC_KEY_SIZE];
pub type Mac = [u8; AUTH_ENC_MAC_SIZE];
pub type Iv = [u8; AUTH_ENC_IV_SIZE];

/// Encryption/Decryption.
pub trait Cryption {
    /// Encrypt a block.
    fn encrypt_block(plaintext: &[u8], key: &Key) -> CipherBlock;

    /// Decrypt a block.
    fn decrypt_block(ciphertext: &[u8], key: &Key, meta: &CipherMeta) -> Result<[u8; BLOCK_SIZE]>;

    /// Encrypt content of any length.
    fn encrypt_arbitrary(plaintext: &[u8], ciphertext: &mut [u8], key: &Key) -> CipherMeta;

    /// Decrypt content of any length.
    fn decrypt_arbitrary(
        ciphertext: &[u8],
        plaintext: &mut [u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<()>;

    /// Encrypt a block using symmetric key cipher(AES128 CTR mode)
    fn symm_encrypt_block(data: &[u8], key: &Key) -> Result<[u8; BLOCK_SIZE]>;

    /// Decrypt a block using symmetric key cipher(AES128 CTR mode)
    fn symm_decrypt_block(data: &[u8], key: &Key) -> Result<[u8; BLOCK_SIZE]>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct CipherMeta {
    mac: Mac,
    // No need to store keys since we use key table to manage keys
    // pub key: Key,
    // No need to store IV since we use different keys to ensure uniqueness
    // pub iv: Iv,
}

#[derive(Clone, Debug)]
pub struct CipherBlock {
    ciphertext: [u8; CIPHER_SIZE],
    cipher_meta: CipherMeta,
}

impl CipherMeta {
    pub fn new(mac: Mac) -> Self {
        Self { mac }
    }

    pub fn mac(&self) -> &Mac {
        &self.mac
    }

    pub fn new_uninit() -> Self {
        Self {
            mac: [0; AUTH_ENC_MAC_SIZE],
        }
    }
}

impl CipherBlock {
    pub fn new_uninit() -> Self {
        Self {
            ciphertext: [0; CIPHER_SIZE],
            cipher_meta: CipherMeta::new_uninit(),
        }
    }

    pub const fn as_slice(&self) -> &[u8] {
        &self.ciphertext
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.ciphertext
    }

    pub const fn cipher_meta(&self) -> &CipherMeta {
        &self.cipher_meta
    }
}

pub struct DefaultCryptor;

impl Cryption for DefaultCryptor {
    fn encrypt_block(plaintext: &[u8], key: &Key) -> CipherBlock {
        Self::enc_block(plaintext, key)
    }

    fn decrypt_block(ciphertext: &[u8], key: &Key, meta: &CipherMeta) -> Result<[u8; BLOCK_SIZE]> {
        Self::dec_block(ciphertext, key, meta)
    }

    fn encrypt_arbitrary(plaintext: &[u8], ciphertext: &mut [u8], key: &Key) -> CipherMeta {
        Self::enc_any(plaintext, ciphertext, key)
    }

    fn decrypt_arbitrary(
        ciphertext: &[u8],
        plaintext: &mut [u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<()> {
        Self::dec_any(ciphertext, plaintext, key, meta)
    }

    fn symm_encrypt_block(data: &[u8], key: &Key) -> Result<[u8; BLOCK_SIZE]> {
        Self::symm_enc_block(data, key)
    }

    fn symm_decrypt_block(data: &[u8], key: &Key) -> Result<[u8; BLOCK_SIZE]> {
        Self::symm_dec_block(data, key)
    }
}

// TODO: Support symmetric encryption for blocks
impl DefaultCryptor {
    #[allow(unused_mut)]
    pub fn gen_random_key() -> Key {
        let mut rand_key = [5u8; AUTH_ENC_KEY_SIZE];
        cfg_if! {
            if #[cfg(feature = "sgx")] {
                thread_rng().fill_bytes(&mut rand_key);
            } else {
                rand_bytes(&mut rand_key);
            }
        }
        rand_key
    }

    #[allow(unused)]
    fn enc_block(plaintext: &[u8], key: &Key) -> CipherBlock {
        debug_assert!(plaintext.len() == BLOCK_SIZE);
        let mut ciphertext: [u8; CIPHER_SIZE] = [0u8; CIPHER_SIZE];
        let mut gmac = [0; 16];

        cfg_if! {
            // AES-GCM
            if #[cfg(feature = "sgx")] {
                let nonce: Iv = [2; 12];
                let aad: [u8; 0] = [0; 0];
                rsgx_rijndael128GCM_encrypt(
                    key,
                    plaintext,
                    &nonce,
                    &aad,
                    &mut ciphertext[..],
                    &mut gmac,
                )
                .unwrap();
            // Fake enc
            } else {
                // TODO: add encryption for non-SGX builds
                ciphertext.copy_from_slice(plaintext);
            }
        }

        CipherBlock {
            ciphertext,
            cipher_meta: CipherMeta { mac: gmac },
        }
    }

    #[allow(unused)]
    fn dec_block(ciphertext: &[u8], key: &Key, meta: &CipherMeta) -> Result<[u8; BLOCK_SIZE]> {
        debug_assert!(ciphertext.len() == BLOCK_SIZE);
        let mut plaintext: [u8; BLOCK_SIZE] = [0u8; BLOCK_SIZE];

        cfg_if! {
            // AES-GCM
            if #[cfg(feature = "sgx")] {
                let gmac = meta.mac;
                let nonce: Iv = [2; 12];
                let aad: [u8; 0] = [0; 0];
                let sgx_res =
                    rsgx_rijndael128GCM_decrypt(key, ciphertext, &nonce, &aad, &gmac, &mut plaintext[..]);
                match sgx_res {
                    Ok(()) => (),
                    Err(sgx_status_t::SGX_ERROR_MAC_MISMATCH) => {
                        return_errno!(EINVAL, "Decryption error, MAC mismatch");
                    },
                    _ => return_errno!(EINVAL, "Unknown decryption error, should not happen"),
                }
            // Fake dec
            } else {
                // TODO: add decryption for non-SGX builds
                plaintext.copy_from_slice(ciphertext);
            }
        }

        Ok(plaintext)
    }

    fn enc_any(plaintext: &[u8], ciphertext: &mut [u8], key: &Key) -> CipherMeta {
        debug_assert!(plaintext.len() == ciphertext.len());
        let mut gmac = [0; 16];
        let nonce: Iv = [2; 12];
        let aad: [u8; 0] = [0; 0];

        cfg_if! {
            // AES-GCM
            if #[cfg(feature = "sgx")] {
                rsgx_rijndael128GCM_encrypt(
                    key,
                    plaintext,
                    &nonce,
                    &aad,
                    ciphertext,
                    &mut gmac,
                )
                .unwrap();
            // Fake enc
            } else {
                let cipher = encrypt_aead(
                    Cipher::aes_128_gcm(),
                    key,
                    Some(&nonce),
                    &aad,
                    plaintext,
                    &mut gmac,
                )
                .unwrap();
                ciphertext.copy_from_slice(&cipher);
            }
        }

        CipherMeta { mac: gmac }
    }

    fn dec_any(
        ciphertext: &[u8],
        plaintext: &mut [u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<()> {
        debug_assert!(ciphertext.len() == plaintext.len());
        let gmac = meta.mac;
        let nonce: Iv = [2; 12];
        let aad: [u8; 0] = [0; 0];

        cfg_if! {
            // AES-GCM
            if #[cfg(feature = "sgx")] {
                let sgx_res =
                    rsgx_rijndael128GCM_decrypt(key, ciphertext, &nonce, &aad, &gmac, plaintext);
                match sgx_res {
                    Ok(()) => (),
                    Err(sgx_status_t::SGX_ERROR_MAC_MISMATCH) => {
                        return_errno!(EINVAL, "Decryption error, MAC mismatch");
                    },
                    _ => return_errno!(EINVAL, "Unknown decryption error, should not happen"),
                }
            // Fake dec
            } else {
                let plain = decrypt_aead(
                    Cipher::aes_128_gcm(),
                    key,
                    Some(&nonce),
                    &aad,
                    &ciphertext,
                    &gmac,
                )
                .unwrap();
                plaintext.copy_from_slice(&plain);
            }
        }
        Ok(())
    }

    fn symm_enc_block(data: &[u8], key: &Key) -> Result<[u8; BLOCK_SIZE]> {
        let mut output = [0u8; BLOCK_SIZE];
        let mut ctr = [2u8; 16];

        cfg_if! {
            // AES128-CTR
            if #[cfg(feature = "sgx")] {
                let ctr_inc_bits = 128u32;
                match rsgx_aes_ctr_encrypt(key, data, &mut ctr, ctr_inc_bits, &mut output) {
                    Ok(_) => (),
                    Err(_) => {
                        return_errno!(EINVAL, "SGX: rsgx_aes_ctr_encrypt error");
                    }
                }
            } else {
                match encrypt(Cipher::aes_128_ctr(), key, Some(&ctr), data) {
                    Ok(ciphertext) => {
                        output.copy_from_slice(&ciphertext);
                    },
                    Err(_) => {
                        return_errno!(EINVAL, "OPENSSL: aes_128_ctr encryption error");
                    }
                }
            }
        }
        Ok(output)
    }

    fn symm_dec_block(data: &[u8], key: &Key) -> Result<[u8; BLOCK_SIZE]> {
        let mut output = [0u8; BLOCK_SIZE];
        let mut ctr = [2u8; 16];

        cfg_if! {
            // AES128-CTR
            if #[cfg(feature = "sgx")] {
                let ctr_inc_bits = 128u32;
                match rsgx_aes_ctr_decrypt(key, data, &mut ctr, ctr_inc_bits, &mut output) {
                    Ok(_) => (),
                    Err(_) => {
                        return_errno!(EINVAL, "SGX: rsgx_aes_ctr_decrypt error");
                    }
                }
            } else {
                match decrypt(Cipher::aes_128_ctr(), key, Some(&ctr), data) {
                    Ok(ciphertext) => {
                        output.copy_from_slice(&ciphertext);
                    },
                    Err(_) => {
                        return_errno!(EINVAL, "OPENSSL: aes_128_ctr decryption error");
                    }
                }
            }
        }
        Ok(output)
    }
}

impl Encoder for Mac {
    fn write_bytes(&mut self, buf: &[u8]) -> Result<()> {
        debug_assert!(self.len() == buf.len());
        self.copy_from_slice(buf);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enc_dec() {
        let key = DefaultCryptor::gen_random_key();

        // encrypt_block & decrypt_block
        let plaintext = [0u8; BLOCK_SIZE];
        let cipher_block = DefaultCryptor::encrypt_block(&plaintext, &key);
        let decrypted = DefaultCryptor::decrypt_block(
            cipher_block.as_slice(),
            &key,
            cipher_block.cipher_meta(),
        )
        .unwrap();
        assert_eq!(plaintext, decrypted);

        // encrypt_arbitrary & decrypt_arbitrary
        const SIZE: usize = 5577;
        let plain = [5u8; SIZE];
        let mut cipher = [0u8; SIZE];
        let cipher_meta = DefaultCryptor::encrypt_arbitrary(&plain, &mut cipher, &key);
        let mut decrypted = [0u8; SIZE];
        let _ = DefaultCryptor::decrypt_arbitrary(&cipher, &mut decrypted, &key, &cipher_meta);
        assert_eq!(plain, decrypted);
    }

    #[test]
    fn test_symm_enc_dec() {
        let key = DefaultCryptor::gen_random_key();
        let plaintext = [0u8; BLOCK_SIZE];
        let ciphertext = DefaultCryptor::symm_encrypt_block(&plaintext, &key).unwrap();
        assert_ne!(plaintext, ciphertext);
        let decrypted = DefaultCryptor::symm_decrypt_block(&ciphertext, &key).unwrap();
        assert_eq!(plaintext, decrypted);
    }
}
