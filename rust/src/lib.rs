#![forbid(unsafe_code)]

pub mod book;
pub mod buffer;
pub mod desk;
pub mod encoding;
pub mod engine;
pub mod evaluation;
pub mod hardware;
pub mod math;
#[cfg(any(feature = "mlx", feature = "mlx-cpu"))]
pub mod mlx;
pub mod model;
pub mod news;
pub mod quant;
pub mod signals;
pub mod simulation;
pub mod storage;
pub mod venue;
pub mod wallet;
pub mod weather;
pub mod polygon {
    include!(concat!(env!("OUT_DIR"), "/polygon.rs"));
}

pub type Result<T> = std::result::Result<T, String>;
