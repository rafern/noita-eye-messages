use crate::utils::run::AnyErrorResult;

pub mod base;
pub mod arx;

pub fn deserialise_cipher(cipher_name: &String, config: &Option<String>) -> AnyErrorResult<impl base::Cipher> {
    match cipher_name.as_str() {
        "arx" => arx::ARXCipher::new(config),
        _ => Err(base::StandardCipherError::UnknownCipher.into()),
    }
}