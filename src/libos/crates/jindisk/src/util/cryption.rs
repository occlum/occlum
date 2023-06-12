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
    /// Encrypt a block. (AEAD)
    fn encrypt_block_aead(plaintext: &[u8], key: &Key) -> CipherBlock;

    /// Decrypt a block. (AEAD)
    fn decrypt_block_aead(
        ciphertext: &[u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<[u8; BLOCK_SIZE]>;

    /// Encrypt content of any length. (AEAD)
    fn encrypt_arbitrary_aead(plaintext: &[u8], ciphertext: &mut [u8], key: &Key) -> CipherMeta;

    /// Decrypt content of any length. (AEAD)
    fn decrypt_arbitrary_aead(
        ciphertext: &[u8],
        plaintext: &mut [u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<()>;

    /// Encrypt a block. (Non-AEAD)
    fn encrypt_block(data: &[u8], key: &Key, iv: Option<&[u8]>) -> Result<[u8; BLOCK_SIZE]>;

    /// Decrypt a block. (Non-AEAD)
    fn decrypt_block(data: &[u8], key: &Key, iv: Option<&[u8]>) -> Result<[u8; BLOCK_SIZE]>;
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
    fn encrypt_block_aead(plaintext: &[u8], key: &Key) -> CipherBlock {
        Self::enc_block_aead(plaintext, key)
    }

    fn decrypt_block_aead(
        ciphertext: &[u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<[u8; BLOCK_SIZE]> {
        Self::dec_block_aead(ciphertext, key, meta)
    }

    fn encrypt_arbitrary_aead(plaintext: &[u8], ciphertext: &mut [u8], key: &Key) -> CipherMeta {
        Self::enc_any_aead(plaintext, ciphertext, key)
    }

    fn decrypt_arbitrary_aead(
        ciphertext: &[u8],
        plaintext: &mut [u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<()> {
        Self::dec_any_aead(ciphertext, plaintext, key, meta)
    }

    fn encrypt_block(data: &[u8], key: &Key, iv: Option<&[u8]>) -> Result<[u8; BLOCK_SIZE]> {
        Self::enc_block(data, key, iv)
    }

    fn decrypt_block(data: &[u8], key: &Key, iv: Option<&[u8]>) -> Result<[u8; BLOCK_SIZE]> {
        Self::dec_block(data, key, iv)
    }
}

impl DefaultCryptor {
    #[allow(unused_mut)]
    pub fn gen_random_key() -> Key {
        let mut rand_key = [5u8; AUTH_ENC_KEY_SIZE];
        cfg_if! {
            if #[cfg(feature = "sgx")] {
                thread_rng().fill_bytes(&mut rand_key);
            } else {
                let _ = rand_bytes(&mut rand_key);
            }
        }
        rand_key
    }

    fn enc_block_aead(plaintext: &[u8], key: &Key) -> CipherBlock {
        debug_assert!(plaintext.len() == BLOCK_SIZE);
        let mut ciphertext: [u8; CIPHER_SIZE] = [0u8; CIPHER_SIZE];
        let mut gmac = [0; AUTH_ENC_MAC_SIZE];
        let nonce: Iv = [2; AUTH_ENC_IV_SIZE];
        let aad: [u8; 0] = [0; 0];

        cfg_if! {
            // AES-GCM-128
            if #[cfg(feature = "sgx")] {
                rsgx_rijndael128GCM_encrypt(
                    key,
                    plaintext,
                    &nonce,
                    &aad,
                    &mut ciphertext[..],
                    &mut gmac,
                )
                .unwrap();
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

        CipherBlock {
            ciphertext,
            cipher_meta: CipherMeta { mac: gmac },
        }
    }

    fn dec_block_aead(ciphertext: &[u8], key: &Key, meta: &CipherMeta) -> Result<[u8; BLOCK_SIZE]> {
        debug_assert!(ciphertext.len() == BLOCK_SIZE);
        let mut plaintext: [u8; BLOCK_SIZE] = [0u8; BLOCK_SIZE];
        let gmac = meta.mac;
        let nonce: Iv = [2; AUTH_ENC_IV_SIZE];
        let aad: [u8; 0] = [0; 0];

        cfg_if! {
            // AES128-GCM
            if #[cfg(feature = "sgx")] {
                let sgx_res =
                    rsgx_rijndael128GCM_decrypt(key, ciphertext, &nonce, &aad, &gmac, &mut plaintext[..]);
                match sgx_res {
                    Ok(()) => (),
                    Err(sgx_status_t::SGX_ERROR_MAC_MISMATCH) => {
                        return_errno!(EINVAL, "Decryption error, MAC mismatch");
                    },
                    _ => return_errno!(EINVAL, "Unknown decryption error, should not happen"),
                }
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

        Ok(plaintext)
    }

    fn enc_any_aead(plaintext: &[u8], ciphertext: &mut [u8], key: &Key) -> CipherMeta {
        debug_assert!(plaintext.len() == ciphertext.len());
        let mut gmac = [0; AUTH_ENC_MAC_SIZE];
        let nonce: Iv = [2; AUTH_ENC_IV_SIZE];
        let aad: [u8; 0] = [0; 0];

        cfg_if! {
            // AES128-GCM
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

    fn dec_any_aead(
        ciphertext: &[u8],
        plaintext: &mut [u8],
        key: &Key,
        meta: &CipherMeta,
    ) -> Result<()> {
        debug_assert!(ciphertext.len() == plaintext.len());
        let gmac = meta.mac;
        let nonce: Iv = [2; AUTH_ENC_IV_SIZE];
        let aad: [u8; 0] = [0; 0];

        cfg_if! {
            // AES128-GCM
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

    #[allow(unused_mut)]
    fn enc_block(data: &[u8], key: &Key, iv: Option<&[u8]>) -> Result<[u8; BLOCK_SIZE]> {
        debug_assert!(data.len() == BLOCK_SIZE);
        let mut output = [0u8; BLOCK_SIZE];
        let mut ctr = {
            const CTR_LEN: usize = 16;
            let mut ctr = [2u8; CTR_LEN];
            if let Some(iv) = iv {
                if iv.len() == CTR_LEN {
                    ctr.copy_from_slice(iv);
                } else {
                    return_errno!(EINVAL, "wrong iv length in AES-CTR");
                }
            }
            ctr
        };

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

    #[allow(unused_mut)]
    fn dec_block(data: &[u8], key: &Key, iv: Option<&[u8]>) -> Result<[u8; BLOCK_SIZE]> {
        debug_assert!(data.len() == BLOCK_SIZE);
        let mut output = [0u8; BLOCK_SIZE];
        let mut ctr = {
            const CTR_LEN: usize = 16;
            let mut ctr = [2u8; CTR_LEN];
            if let Some(iv) = iv {
                if iv.len() == CTR_LEN {
                    ctr.copy_from_slice(iv);
                } else {
                    return_errno!(EINVAL, "wrong iv length in AES-CTR");
                }
            }
            ctr
        };

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
    fn test_enc_dec_aead() {
        let key = DefaultCryptor::gen_random_key();

        // encrypt_block & decrypt_block
        let plaintext = [0u8; BLOCK_SIZE];
        let cipher_block = DefaultCryptor::encrypt_block_aead(&plaintext, &key);
        let decrypted = DefaultCryptor::decrypt_block_aead(
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
        let cipher_meta = DefaultCryptor::encrypt_arbitrary_aead(&plain, &mut cipher, &key);
        let mut decrypted = [0u8; SIZE];
        let _ = DefaultCryptor::decrypt_arbitrary_aead(&cipher, &mut decrypted, &key, &cipher_meta);
        assert_eq!(plain, decrypted);
    }

    #[test]
    fn test_enc_dec() {
        let key = DefaultCryptor::gen_random_key();
        let plaintext = [0u8; BLOCK_SIZE];
        let ciphertext = DefaultCryptor::encrypt_block(&plaintext, &key, None).unwrap();
        assert_ne!(plaintext, ciphertext);
        let decrypted = DefaultCryptor::decrypt_block(&ciphertext, &key, None).unwrap();
        assert_eq!(plaintext, decrypted);
    }
}
