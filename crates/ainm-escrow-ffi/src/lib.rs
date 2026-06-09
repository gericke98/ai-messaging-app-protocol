//! UniFFI surface over `ainm-escrow` for the iOS client — the SAME crate the server uses,
//! so escrow recovery is wire-compatible by construction.
uniffi::setup_scaffolding!();

use ainm_escrow::{client_blind, client_finalize_and_open, enroll, BlindState, KdfParams};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

#[derive(uniffi::Record)]
pub struct EnrollResult {
    pub oprf_key_b64: String,
    pub ciphertext_b64: String,
    pub kdf_params_json: String,
}

#[derive(uniffi::Error, Debug, thiserror::Error)]
pub enum EscrowFfiError {
    #[error("{0}")]
    Crypto(String),
}

/// Enrollment: derive the wrapping key + seal the 32-byte secret. Returns the 3 blobs to upload.
#[uniffi::export]
pub fn escrow_enroll(secret: Vec<u8>, pin: String) -> Result<EnrollResult, EscrowFfiError> {
    let secret: [u8; 32] = secret
        .try_into()
        .map_err(|_| EscrowFfiError::Crypto("secret must be 32 bytes".into()))?;
    let e = enroll(&secret, &pin).map_err(EscrowFfiError::Crypto)?;
    Ok(EnrollResult {
        oprf_key_b64: URL_SAFE_NO_PAD.encode(&e.oprf_key),
        ciphertext_b64: URL_SAFE_NO_PAD.encode(&e.ciphertext),
        kdf_params_json: serde_json::to_string(&e.kdf_params)
            .map_err(|err| EscrowFfiError::Crypto(err.to_string()))?,
    })
}

/// Opaque handle holding the blind state between blind() and finalize() across the network round-trip.
#[derive(uniffi::Object)]
pub struct EscrowRecovery {
    inner: std::sync::Mutex<Option<BlindState>>,
    kdf_json: String,
    pin: String,
}

#[uniffi::export]
impl EscrowRecovery {
    /// Step 1: blind the PIN. `kdf_params_json` comes from the server (recover/params).
    #[uniffi::constructor]
    pub fn begin(
        pin: String,
        kdf_params_json: String,
    ) -> Result<std::sync::Arc<Self>, EscrowFfiError> {
        let kdf: KdfParams = serde_json::from_str(&kdf_params_json)
            .map_err(|e| EscrowFfiError::Crypto(e.to_string()))?;
        let blind = client_blind(&pin, &kdf).map_err(EscrowFfiError::Crypto)?;
        Ok(std::sync::Arc::new(Self {
            inner: std::sync::Mutex::new(Some(blind)),
            kdf_json: kdf_params_json,
            pin,
        }))
    }

    /// The base64url blinded point to POST as `blinded_point`.
    pub fn blinded_point(&self) -> Result<String, EscrowFfiError> {
        let g = self.inner.lock().unwrap();
        g.as_ref()
            .map(|b| b.blinded_b64.clone())
            .ok_or(EscrowFfiError::Crypto("already finalized".into()))
    }

    /// Step 2: finalize with the server's evaluated point + ciphertext; returns the 32-byte secret.
    pub fn finalize(
        &self,
        evaluated_point: String,
        ciphertext_b64: String,
    ) -> Result<Vec<u8>, EscrowFfiError> {
        let blind = self
            .inner
            .lock()
            .unwrap()
            .take()
            .ok_or(EscrowFfiError::Crypto("already finalized".into()))?;
        let kdf: KdfParams = serde_json::from_str(&self.kdf_json)
            .map_err(|e| EscrowFfiError::Crypto(e.to_string()))?;
        let ct = URL_SAFE_NO_PAD
            .decode(&ciphertext_b64)
            .map_err(|e| EscrowFfiError::Crypto(e.to_string()))?;
        let secret = client_finalize_and_open(&self.pin, &kdf, &blind, &evaluated_point, &ct)
            .map_err(EscrowFfiError::Crypto)?;
        Ok(secret.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ffi_enroll_then_local_recover() {
        let secret = vec![5u8; 32];
        let pin = "314159".to_string();
        let e = escrow_enroll(secret.clone(), pin.clone()).unwrap();
        let oprf_key = URL_SAFE_NO_PAD.decode(&e.oprf_key_b64).unwrap();
        let rec = EscrowRecovery::begin(pin, e.kdf_params_json.clone()).unwrap();
        let blinded = rec.blinded_point().unwrap();
        let evaluated = ainm_escrow::server_evaluate(&oprf_key, &blinded).unwrap();
        let recovered = rec.finalize(evaluated, e.ciphertext_b64).unwrap();
        assert_eq!(recovered, secret);
        // wrong PIN fails
        let e2 = escrow_enroll(vec![9u8; 32], "111111".to_string()).unwrap();
        let oprf2 = URL_SAFE_NO_PAD.decode(&e2.oprf_key_b64).unwrap();
        let rec2 = EscrowRecovery::begin("222222".to_string(), e2.kdf_params_json).unwrap();
        let ev2 = ainm_escrow::server_evaluate(&oprf2, &rec2.blinded_point().unwrap()).unwrap();
        assert!(rec2.finalize(ev2, e2.ciphertext_b64).is_err());
    }
}
