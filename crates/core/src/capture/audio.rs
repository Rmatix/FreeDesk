//! WASAPI Loopback system audio capture for Windows, with cross-platform fallback.
//! Captures all output audio playing on the host PC in real time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
#[cfg(windows)]
use tracing::error;
use tracing::info;

pub struct AudioLoopbackCapture {
    running: Arc<AtomicBool>,
}

impl Default for AudioLoopbackCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioLoopbackCapture {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts streaming host audio chunks onto the provided MPSC sender
    pub fn start(&self, tx: mpsc::Sender<crate::protocol::AudioChunk>) -> anyhow::Result<()> {
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        #[cfg(windows)]
        {
            std::thread::spawn(move || {
                unsafe {
                    let _ = windows::Win32::System::Com::CoInitializeEx(
                        None,
                        windows::Win32::System::Com::COINIT_MULTITHREADED,
                    );
                }

                if let Err(e) = run_audio_loopback(&running, tx) {
                    error!("Audio loopback capture thread exited with error: {:?}", e);
                }

                unsafe {
                    windows::Win32::System::Com::CoUninitialize();
                }
            });

            info!("WASAPI Loopback Audio Capture thread launched successfully.");
        }

        #[cfg(not(windows))]
        {
            let _ = tx;
            info!("Audio loopback capture is currently Windows-only; non-Windows fallback active.");
        }

        Ok(())
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(windows)]
fn run_audio_loopback(
    running: &Arc<AtomicBool>,
    tx: mpsc::Sender<crate::protocol::AudioChunk>,
) -> anyhow::Result<()> {
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole)?;
        let audio_client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

        let mix_format_ptr = audio_client.GetMixFormat()?;
        let mix_format = &*mix_format_ptr;

        let sample_rate = mix_format.nSamplesPerSec;
        let channels = mix_format.nChannels;
        let block_align = mix_format.nBlockAlign as usize;

        // 100ms buffer size
        let hns_requested = 1000000;
        audio_client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_LOOPBACK,
            hns_requested,
            0,
            mix_format_ptr,
            None,
        )?;

        let capture_client: IAudioCaptureClient = audio_client.GetService()?;
        audio_client.Start()?;

        while running.load(Ordering::SeqCst) {
            let mut packet_length = capture_client.GetNextPacketSize()?;
            while packet_length > 0 {
                let mut data_ptr = std::ptr::null_mut();
                let mut frames_available = 0u32;
                let mut flags = 0u32;

                capture_client.GetBuffer(
                    &mut data_ptr,
                    &mut frames_available,
                    &mut flags,
                    None,
                    None,
                )?;

                let bytes_len = frames_available as usize * block_align;
                let mut pcm_bytes = vec![0u8; bytes_len];

                // If silent flag AUDCLNT_BUFFERFLAGS_SILENT (2) is set, leave as zeros
                if (flags & 2) == 0 && !data_ptr.is_null() {
                    std::ptr::copy_nonoverlapping(data_ptr, pcm_bytes.as_mut_ptr(), bytes_len);
                }

                capture_client.ReleaseBuffer(frames_available)?;

                if !pcm_bytes.is_empty() {
                    let chunk = crate::protocol::AudioChunk {
                        sample_rate,
                        channels,
                        pcm_data: pcm_bytes,
                    };
                    if tx.blocking_send(chunk).is_err() {
                        break;
                    }
                }

                packet_length = capture_client.GetNextPacketSize()?;
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let _ = audio_client.Stop();
    }

    Ok(())
}
