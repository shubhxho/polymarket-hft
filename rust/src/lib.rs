#![forbid(unsafe_code)]

pub mod book;
pub mod engine;
#[cfg(feature = "mlx")]
pub mod mlx;
pub mod model;
pub mod quant;
pub mod base {
    include!(concat!(env!("OUT_DIR"), "/base.rs"));
}

pub type Result<T> = std::result::Result<T, String>;
