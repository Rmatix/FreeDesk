#[cfg(windows)]
pub mod simulator;

#[cfg(windows)]
pub use simulator::InputSimulator;
