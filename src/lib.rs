// mod object; // Removed
mod encode;
pub mod decode;
pub mod types;
pub mod value;

// #[derive(Debug, thiserror::Error)]
// pub enum Error {
//     #[error("Failed to read varint")]
//     VarintReadError(#[from] ),
// }


pub type Result<T> = std::result::Result<T, std::io::Error>;

pub use decode::{Decoder, GobDecodable};
pub use encode::{Encoder, GobEncodable, encode_as_interface};
pub use value::Value;

// Re-export macro
pub use gob_macro::Gob;
pub use gob_macro::Gob as gob;

pub trait GobType {
    const ID: i64;
}

#[macro_export]
macro_rules! define_type_id {
    ($name:ty, $id:expr) => {
        impl $crate::GobType for $name {
            const ID: i64 = $id;
        }
    };
}
