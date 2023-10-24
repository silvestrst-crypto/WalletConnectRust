use anyhow::Result;
use base64::prelude::BASE64_STANDARD;
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng, Payload};
use chacha20poly1305::{AeadCore, ChaCha20Poly1305, Nonce};

const TYPE_0: u8 = 0;
const TYPE_1: u8 = 1;
const TYPE_INDEX: usize = 0;
const TYPE_LENGTH: usize = 1;
const IV_LENGTH: usize = 12;
const PUB_KEY_LENGTH: usize = 32;
const SYM_KEY_LENGTH: usize = 32;

pub type Iv = [u8; IV_LENGTH];
pub type SymKey = [u8; SYM_KEY_LENGTH];
pub type PubKey = [u8; PUB_KEY_LENGTH];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvelopeType<'a> {
    Type0,
    Type1 { sender_public_key: &'a PubKey },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EncodingParams<'a> {
    sealed: &'a [u8],
    iv: &'a Iv,
    envelope_type: EnvelopeType<'a>,
}

impl<'a> EncodingParams<'a> {
    fn parse_decoded(data: &'a [u8]) -> Result<Self> {
        let envelope_type = data[0];
        match envelope_type {
            TYPE_0 => {
                let iv_index: usize = TYPE_INDEX + TYPE_LENGTH;
                let sealed_index: usize = iv_index + IV_LENGTH;
                Ok(EncodingParams {
                    iv: data[iv_index..=IV_LENGTH].try_into()?,
                    sealed: &data[sealed_index..],
                    envelope_type: EnvelopeType::Type0,
                })
            }
            TYPE_1 => {
                let key_index: usize = TYPE_INDEX + TYPE_LENGTH;
                let iv_index: usize = key_index + PUB_KEY_LENGTH;
                let sealed_index: usize = iv_index + IV_LENGTH;
                Ok(EncodingParams {
                    iv: data[iv_index..=IV_LENGTH].try_into()?,
                    sealed: &data[sealed_index..],
                    envelope_type: EnvelopeType::Type1 {
                        sender_public_key: data[key_index..=PUB_KEY_LENGTH].try_into()?,
                    },
                })
            }
            _ => anyhow::bail!("Invalid envelope type: {}", envelope_type),
        }
    }
}

// TODO: RNG as an input
pub fn encrypt_and_encode<T>(envelope_type: EnvelopeType, msg: T, key: &SymKey) -> Result<String>
where
    T: AsRef<[u8]>,
{
    let payload = Payload {
        msg: msg.as_ref(),
        aad: &[],
    };
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let sealed = encrypt(&nonce, payload, key)?;
    Ok(encode(
        envelope_type,
        sealed.as_slice(),
        nonce.as_slice().try_into()?,
    ))
}

pub fn decode_and_decrypt_type0<T>(msg: T, key: &SymKey) -> Result<String>
where
    T: AsRef<[u8]>,
{
    let data = BASE64_STANDARD.decode(msg)?;
    let decoded = EncodingParams::parse_decoded(&data)?;
    if let EnvelopeType::Type1 { .. } = decoded.envelope_type {
        anyhow::bail!("Expected envelope type 0");
    }

    let payload = Payload {
        msg: decoded.sealed,
        aad: &[],
    };
    let decrypted = decrypt(decoded.iv.try_into()?, payload, key)?;

    Ok(String::from_utf8(decrypted)?)
}

fn encrypt(nonce: &Nonce, payload: Payload<'_, '_>, key: &SymKey) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(key.try_into()?);
    let sealed = cipher
        .encrypt(nonce, payload)
        .map_err(|e| anyhow::anyhow!("Encryption failed, err: {:?}", e))?;

    Ok(sealed)
}

fn encode(envelope_type: EnvelopeType, sealed: &[u8], iv: &Iv) -> String {
    match envelope_type {
        EnvelopeType::Type0 => BASE64_STANDARD.encode([&[TYPE_0], iv.as_slice(), sealed].concat()),
        EnvelopeType::Type1 { sender_public_key } => {
            BASE64_STANDARD.encode([&[TYPE_1], sender_public_key.as_slice(), iv, sealed].concat())
        }
    }
}

fn decrypt(nonce: &Nonce, payload: Payload<'_, '_>, key: &SymKey) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(key.try_into()?);
    let unsealed = cipher
        .decrypt(nonce, payload)
        .map_err(|e| anyhow::anyhow!("Decryption failed, err: {:?}", e))?;

    Ok(unsealed)
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::*;

    // https://www.rfc-editor.org/rfc/rfc7539#section-2.8.2
    // Below constans are taken from this section of the RFC.

    const PLAINTEXT: &str = r#"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it."#;
    const CIPHERTEXT: [u8; 114] = hex!(
        "d3 1a 8d 34 64 8e 60 db 7b 86 af bc 53 ef 7e c2
         a4 ad ed 51 29 6e 08 fe a9 e2 b5 a7 36 ee 62 d6
         3d be a4 5e 8c a9 67 12 82 fa fb 69 da 92 72 8b
         1a 71 de 0a 9e 06 0b 29 05 d6 a5 b6 7e cd 3b 36
         92 dd bd 7f 2d 77 8b 8c 98 03 ae e3 28 09 1b 58
         fa b3 24 e4 fa d6 75 94 55 85 80 8b 48 31 d7 bc
         3f f4 de f0 8e 4b 7a 9d e5 76 d2 65 86 ce c6 4b
         61 16"
    );
    const TAG: [u8; 16] = hex!("1a e1 0b 59 4f 09 e2 6a 7e 90 2e cb d0 60 06 91");
    const SYMKEY: SymKey = hex!(
        "80 81 82 83 84 85 86 87 88 89 8a 8b 8c 8d 8e 8f
         90 91 92 93 94 95 96 97 98 99 9a 9b 9c 9d 9e 9f"
    );
    const AAD: [u8; 12] = hex!("50 51 52 53 c0 c1 c2 c3 c4 c5 c6 c7");
    const IV: Iv = hex!("07 00 00 00 40 41 42 43 44 45 46 47");

    /// Tests WCv2 encoding and decoding.
    #[test]
    fn test_decode_encoded() -> Result<()> {
        let iv: &Iv = IV.as_slice().try_into()?;
        let sealed = [CIPHERTEXT.as_slice(), TAG.as_slice()].concat();

        let encoded = encode(EnvelopeType::Type0, &sealed, iv);
        assert_eq!(
            encoded,
            "AAcAAABAQUJDREVGR9MajTRkjmDbe4avvFPvfsKkre1RKW4I/qnitac27mLWPb6kXoypZxKC+vtp2pJyixpx3gqeBgspBdaltn7NOzaS3b1/LXeLjJgDruMoCRtY+rMk5PrWdZRVhYCLSDHXvD/03vCOS3qd5XbSZYbOxkthFhrhC1lPCeJqfpAuy9BgBpE="
        );

        let data = BASE64_STANDARD.decode(&encoded)?;
        let decoded = EncodingParams::parse_decoded(&data)?;
        assert_eq!(decoded.envelope_type, EnvelopeType::Type0);
        assert_eq!(decoded.sealed, sealed);
        assert_eq!(decoded.iv, iv);

        Ok(())
    }

    /// Tests ChaCha20-Poly1305 encryption against the RFC test vector.
    ///
    /// https://www.rfc-editor.org/rfc/rfc7539#section-2.8.2
    /// Please note that this test vector has an
    /// "Additional Authentication Data", in practice, we will likely
    /// be using this algorithm without "AAD".
    #[test]
    fn test_encryption() -> Result<()> {
        let payload = Payload {
            msg: PLAINTEXT.as_bytes(),
            aad: AAD.as_slice(),
        };
        let iv = IV.as_slice().try_into()?;

        let sealed = encrypt(iv, payload, &SYMKEY)?;
        assert_eq!(sealed, [CIPHERTEXT.as_slice(), TAG.as_slice()].concat());

        Ok(())
    }

    /// Tests that encrypted message can be decrypted back.
    #[test]
    fn test_decrypt_encrypted() -> Result<()> {
        let iv = IV.as_slice().try_into()?;

        let seal_payload = Payload {
            msg: PLAINTEXT.as_bytes(),
            aad: AAD.as_slice(),
        };
        let sealed = encrypt(iv, seal_payload, &SYMKEY)?;

        let unseal_payload = Payload {
            msg: &sealed,
            aad: AAD.as_slice(),
        };
        let unsealed = decrypt(iv, unseal_payload, &SYMKEY)?;

        assert_eq!(PLAINTEXT.to_string(), String::from_utf8(unsealed)?);

        Ok(())
    }

    /// Tests that plain text can be WCv2 serialized and deserialized back.
    #[test]
    fn test_encrypt_encode_decode_decrypt() -> Result<()> {
        let encoded = encrypt_and_encode(EnvelopeType::Type0, PLAINTEXT, &SYMKEY)?;
        let decoded = decode_and_decrypt_type0(&encoded, &SYMKEY)?;
        assert_eq!(decoded, PLAINTEXT);

        Ok(())
    }
}
