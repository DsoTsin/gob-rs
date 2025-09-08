mod object;
mod encode;
mod decode;

// #[derive(Debug, thiserror::Error)]
// pub enum Error {
//     #[error("Failed to read varint")]
//     VarintReadError(#[from] ),
// }


pub type Result<T> = std::result::Result<T, std::io::Error>;

pub use decode::Decoder;