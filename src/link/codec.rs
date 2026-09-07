use super::LinkEnvelope;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum CodecError {
    #[error("serialization error: {reason}")]
    Serialization { reason: String },
}

pub fn encode(envelope: &LinkEnvelope) -> Result<Vec<u8>, CodecError> {
    serde_json::to_vec(envelope).map_err(|error| CodecError::Serialization {
        reason: error.to_string(),
    })
}

pub fn decode(bytes: &[u8]) -> Result<LinkEnvelope, CodecError> {
    serde_json::from_slice(bytes).map_err(|error| CodecError::Serialization {
        reason: error.to_string(),
    })
}
