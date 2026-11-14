// SPDX-License-Identifier: GPL-3.0-or-later
//! OSSL crypto functions: SHA-256, AES-256-CBC encrypt/decrypt.

use std::sync::Arc;

use aes::Aes256;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use cbc::cipher::KeyIvInit;
use cbc::cipher::block_padding::NoPadding;
use cbc::cipher::{BlockEncryptMut, BlockDecryptMut};
use hmac::Hmac;
use mlua::prelude::*;
use parking_lot::RwLock;
use sha2::{Digest, Sha256};

use crate::builtins::ScriptContext;
use super::{OsslFunc, OsslThreatLevel};

type Aes256CbcEnc = cbc::Encryptor<Aes256>;
type Aes256CbcDec = cbc::Decryptor<Aes256>;

pub(crate) fn crypto_functions() -> Vec<OsslFunc> {
    vec![
        OsslFunc {
            name: "osSHA256",
            level: OsslThreatLevel::VeryLow,
            register: register_os_sha256,
        },
        OsslFunc {
            name: "osAESEncrypt",
            level: OsslThreatLevel::Low,
            register: register_os_aes_encrypt,
        },
        OsslFunc {
            name: "osAESDecrypt",
            level: OsslThreatLevel::Low,
            register: register_os_aes_decrypt,
        },
        OsslFunc {
            name: "osAESEncryptTo",
            level: OsslThreatLevel::Low,
            register: register_os_aes_encrypt_to,
        },
        OsslFunc {
            name: "osAESDecryptFrom",
            level: OsslThreatLevel::Low,
            register: register_os_aes_decrypt_from,
        },
    ]
}

// ---------------------------------------------------------------------------
// Key derivation: PBKDF2 with SHA-256 (matches C# Rfc2898DeriveBytes)
// ---------------------------------------------------------------------------

/// Default salt for PBKDF2 when no IV is given.
const DEFAULT_SALT: &[u8] = b"OpenSimulator OSSL";
/// Default iteration count (matching C# defaults).
const PBKDF2_ITERATIONS: u32 = 1000;
/// AES-256 key length.
const KEY_LEN: usize = 32;
/// AES block / IV length.
const IV_LEN: usize = 16;

/// Derive a 32-byte key and 16-byte IV from a passphrase using PBKDF2-HMAC-SHA256.
fn derive_key_iv(passphrase: &str, salt: &[u8]) -> ([u8; KEY_LEN], [u8; IV_LEN]) {
    let mut key = [0u8; KEY_LEN];
    let mut iv = [0u8; IV_LEN];
    // Derive key + iv in one pass (48 bytes)
    let mut buf = [0u8; KEY_LEN + IV_LEN];
    pbkdf2::pbkdf2::<Hmac<Sha256>>(
        passphrase.as_bytes(),
        salt,
        PBKDF2_ITERATIONS,
        &mut buf,
    )
    .expect("PBKDF2 should not fail with valid params");
    key.copy_from_slice(&buf[..KEY_LEN]);
    iv.copy_from_slice(&buf[KEY_LEN..]);
    (key, iv)
}

/// PKCS7 padding: add 1..16 bytes so length is a multiple of 16.
fn pkcs7_pad(data: &[u8]) -> Vec<u8> {
    let pad_len = IV_LEN - (data.len() % IV_LEN);
    let mut padded = Vec::with_capacity(data.len() + pad_len);
    padded.extend_from_slice(data);
    padded.extend(std::iter::repeat_n(pad_len as u8, pad_len));
    padded
}

/// Remove PKCS7 padding. Returns an error on invalid padding.
fn pkcs7_unpad(data: &[u8]) -> Result<&[u8], &'static str> {
    if data.is_empty() {
        return Err("empty data");
    }
    let pad_byte = *data.last().unwrap();
    if pad_byte == 0 || pad_byte as usize > IV_LEN || pad_byte as usize > data.len() {
        return Err("invalid PKCS7 padding");
    }
    let pad_start = data.len() - pad_byte as usize;
    for &b in &data[pad_start..] {
        if b != pad_byte {
            return Err("invalid PKCS7 padding");
        }
    }
    Ok(&data[..pad_start])
}

/// Encrypt data using AES-256-CBC. Data must already be PKCS7-padded
/// (length is a multiple of 16).
fn aes_cbc_encrypt(key: &[u8; KEY_LEN], iv: &[u8; IV_LEN], padded_data: &[u8]) -> Vec<u8> {
    let cipher = Aes256CbcEnc::new(key.into(), iv.into());
    let mut buf = padded_data.to_vec();
    // encrypt_padded_mut with NoPadding: data must already be block-aligned.
    let ct = cipher
        .encrypt_padded_mut::<NoPadding>(&mut buf, padded_data.len())
        .expect("encrypt: padded data should be block-aligned");
    ct.to_vec()
}

/// Decrypt AES-256-CBC ciphertext. Returns the raw decrypted bytes
/// (still PKCS7-padded).
fn aes_cbc_decrypt(
    key: &[u8; KEY_LEN],
    iv: &[u8; IV_LEN],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = Aes256CbcDec::new(key.into(), iv.into());
    let mut buf = ciphertext.to_vec();
    let pt = cipher
        .decrypt_padded_mut::<NoPadding>(&mut buf)
        .map_err(|e| format!("decryption failed: {e}"))?;
    Ok(pt.to_vec())
}

// ---------------------------------------------------------------------------
// osSHA256
// ---------------------------------------------------------------------------

fn register_os_sha256(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, data: String| {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let result = hasher.finalize();
        Ok(hex::encode(result))
    })?;
    lua.globals().set("osSHA256", f)
}

// ---------------------------------------------------------------------------
// osAESEncrypt / osAESDecrypt (passphrase-derived key, default salt)
// ---------------------------------------------------------------------------

fn register_os_aes_encrypt(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (key_passphrase, data): (String, String)| {
        let (key, iv) = derive_key_iv(&key_passphrase, DEFAULT_SALT);
        let padded = pkcs7_pad(data.as_bytes());
        let ciphertext = aes_cbc_encrypt(&key, &iv, &padded);
        Ok(BASE64.encode(&ciphertext))
    })?;
    lua.globals().set("osAESEncrypt", f)
}

fn register_os_aes_decrypt(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(|_, (key_passphrase, data_b64): (String, String)| {
        let (key, iv) = derive_key_iv(&key_passphrase, DEFAULT_SALT);
        let ciphertext = BASE64.decode(data_b64.as_bytes()).map_err(|e| {
            mlua::Error::RuntimeError(format!("osAESDecrypt: bad base64: {e}"))
        })?;

        let decrypted = aes_cbc_decrypt(&key, &iv, &ciphertext).map_err(|e| {
            mlua::Error::RuntimeError(format!("osAESDecrypt: {e}"))
        })?;

        let unpadded = pkcs7_unpad(&decrypted).map_err(|e| {
            mlua::Error::RuntimeError(format!("osAESDecrypt: {e}"))
        })?;

        String::from_utf8(unpadded.to_vec()).map_err(|e| {
            mlua::Error::RuntimeError(format!("osAESDecrypt: invalid UTF-8: {e}"))
        })
    })?;
    lua.globals().set("osAESDecrypt", f)
}

// ---------------------------------------------------------------------------
// osAESEncryptTo / osAESDecryptFrom (explicit IV)
// ---------------------------------------------------------------------------

fn register_os_aes_encrypt_to(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (key_passphrase, iv_b64, data): (String, String, String)| {
            let iv_bytes = BASE64.decode(iv_b64.as_bytes()).map_err(|e| {
                mlua::Error::RuntimeError(format!("osAESEncryptTo: bad IV base64: {e}"))
            })?;
            if iv_bytes.len() != IV_LEN {
                return Err(mlua::Error::RuntimeError(format!(
                    "osAESEncryptTo: IV must be {IV_LEN} bytes, got {}",
                    iv_bytes.len()
                )));
            }

            let (key, _) = derive_key_iv(&key_passphrase, DEFAULT_SALT);
            let padded = pkcs7_pad(data.as_bytes());

            let mut iv_arr = [0u8; IV_LEN];
            iv_arr.copy_from_slice(&iv_bytes);

            let ciphertext = aes_cbc_encrypt(&key, &iv_arr, &padded);
            Ok(BASE64.encode(&ciphertext))
        },
    )?;
    lua.globals().set("osAESEncryptTo", f)
}

fn register_os_aes_decrypt_from(lua: &Lua, _ctx: &Arc<RwLock<ScriptContext>>) -> LuaResult<()> {
    let f = lua.create_function(
        |_, (key_passphrase, iv_b64, data_b64): (String, String, String)| {
            let iv_bytes = BASE64.decode(iv_b64.as_bytes()).map_err(|e| {
                mlua::Error::RuntimeError(format!("osAESDecryptFrom: bad IV base64: {e}"))
            })?;
            if iv_bytes.len() != IV_LEN {
                return Err(mlua::Error::RuntimeError(format!(
                    "osAESDecryptFrom: IV must be {IV_LEN} bytes, got {}",
                    iv_bytes.len()
                )));
            }
            let ciphertext = BASE64.decode(data_b64.as_bytes()).map_err(|e| {
                mlua::Error::RuntimeError(format!("osAESDecryptFrom: bad data base64: {e}"))
            })?;

            let (key, _) = derive_key_iv(&key_passphrase, DEFAULT_SALT);
            let mut iv_arr = [0u8; IV_LEN];
            iv_arr.copy_from_slice(&iv_bytes);

            let decrypted = aes_cbc_decrypt(&key, &iv_arr, &ciphertext).map_err(|e| {
                mlua::Error::RuntimeError(format!("osAESDecryptFrom: {e}"))
            })?;

            let unpadded = pkcs7_unpad(&decrypted).map_err(|e| {
                mlua::Error::RuntimeError(format!("osAESDecryptFrom: {e}"))
            })?;

            String::from_utf8(unpadded.to_vec()).map_err(|e| {
                mlua::Error::RuntimeError(format!("osAESDecryptFrom: invalid UTF-8: {e}"))
            })
        },
    )?;
    lua.globals().set("osAESDecryptFrom", f)
}

// ---------------------------------------------------------------------------
// Hex encoding helper (avoid bringing in a whole crate for this)
// ---------------------------------------------------------------------------

mod hex {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    pub fn encode(data: impl AsRef<[u8]>) -> String {
        let data = data.as_ref();
        let mut result = String::with_capacity(data.len() * 2);
        for &byte in data {
            result.push(HEX_CHARS[(byte >> 4) as usize] as char);
            result.push(HEX_CHARS[(byte & 0xf) as usize] as char);
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::ScriptContext;
    use crate::ossl::{OsslConfig, OsslThreatLevel, register_ossl};
    use uuid::Uuid;

    fn make_ctx() -> Arc<RwLock<ScriptContext>> {
        Arc::new(RwLock::new(ScriptContext {
            script_id: Uuid::new_v4(),
            prim_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            ..ScriptContext::default()
        }))
    }

    fn make_crypto_vm() -> Lua {
        let lua = Lua::new();
        let ctx = make_ctx();
        let config = OsslConfig {
            max_threat_level: OsslThreatLevel::Low,
            ..Default::default()
        };
        register_ossl(&lua, &config, &ctx).unwrap();
        lua
    }

    #[test]
    fn test_sha256() {
        let lua = make_crypto_vm();
        let r: String = lua.load(r#"return osSHA256("hello")"#).eval().unwrap();
        assert_eq!(
            r,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_sha256_empty() {
        let lua = make_crypto_vm();
        let r: String = lua.load(r#"return osSHA256("")"#).eval().unwrap();
        assert_eq!(
            r,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_aes_encrypt_decrypt_roundtrip() {
        let lua = make_crypto_vm();
        let r: String = lua
            .load(
                r#"
                local encrypted = osAESEncrypt("mypassword", "Hello World")
                return osAESDecrypt("mypassword", encrypted)
            "#,
            )
            .eval()
            .unwrap();
        assert_eq!(r, "Hello World");
    }

    #[test]
    fn test_aes_encrypt_to_decrypt_from_roundtrip() {
        let lua = make_crypto_vm();
        let r: String = lua
            .load(
                r#"
                local iv = "AAAAAAAAAAAAAAAAAAAAAA=="
                local encrypted = osAESEncryptTo("mypassword", iv, "Secret Data")
                return osAESDecryptFrom("mypassword", iv, encrypted)
            "#,
            )
            .eval()
            .unwrap();
        assert_eq!(r, "Secret Data");
    }

    #[test]
    fn test_aes_different_keys_produce_different_output() {
        let lua = make_crypto_vm();
        let r: Vec<String> = lua
            .load(
                r#"
                local e1 = osAESEncrypt("key1", "data")
                local e2 = osAESEncrypt("key2", "data")
                return {e1, e2}
            "#,
            )
            .eval()
            .unwrap();
        assert_ne!(r[0], r[1]);
    }
}
