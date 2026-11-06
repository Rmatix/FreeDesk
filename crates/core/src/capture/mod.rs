#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod screen;

#[cfg(windows)]
pub use audio::AudioLoopbackCapture;
#[cfg(windows)]
pub use screen::ScreenCapture;
