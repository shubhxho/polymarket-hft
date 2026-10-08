#![forbid(unsafe_code)]

pub mod book;
pub mod engine;
#[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
pub mod mlx;
pub mod model;
pub mod news;
pub mod quant;
pub mod storage;
pub mod base {
    include!(concat!(env!("OUT_DIR"), "/base.rs"));
}

pub type Result<T> = std::result::Result<T, String>;
