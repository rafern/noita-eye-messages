use crate::utils::run::AnyErrorResult;

pub mod base;
pub mod arx;

pub fn deserialise_cipher(cipher_name: &str, config: Option<&str>) -> AnyErrorResult<impl base::Cipher> {
    match cipher_name {
        "arx" => arx::ARXCipher::new(config),
        _ => Err(base::StandardCipherError::UnknownCipher.into()),
    }
}