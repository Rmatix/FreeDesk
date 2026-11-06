//! FreeDesk Core Library
//! High-performance, zero-latency, secure remote desktop building blocks.

pub mod capture;
pub mod codec;
pub mod crypto;
pub mod input;
pub mod protocol;
pub mod security;

pub fn version() -> &'static str {
    "0.1.0"
}
