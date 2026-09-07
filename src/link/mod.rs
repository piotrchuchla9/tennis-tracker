mod codec;
mod message;

pub use codec::{decode, encode, CodecError};
pub use message::{LinkEnvelope, LinkMessage};
