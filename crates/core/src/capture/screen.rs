//! Screen capture implementation for Windows.
//! Provides fast desktop frame buffer acquisition.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP, HDC, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

pub struct ScreenCapture {
    pub width: u32,
    pub height: u32,
    hdc_screen: HDC,
    hdc_mem: HDC,
    h_bitmap: HBITMAP,
    bmi: BITMAPINFO,
}

// Safety: HDC and HBITMAP handles wrapped with teardown logic
unsafe impl Send for ScreenCapture {}

impl ScreenCapture {
    pub fn new() -> anyhow::Result<Self> {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN) as u32;
            let height = GetSystemMetrics(SM_CYSCREEN) as u32;

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            if hdc_screen.0.is_null() {
                anyhow::bail!("Failed to get desktop device context");
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let h_bitmap = CreateCompatibleBitmap(hdc_screen, width as i32, height as i32);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // Top-down DIB
                    biPlanes: 1,
                    biBitCount: 32, // BGRA 32-bit
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default(); 1],
            };

            SelectObject(hdc_mem, h_bitmap);

            Ok(Self {
                width,
                height,
                hdc_screen,
                hdc_mem,
                h_bitmap,
                bmi,
            })
        }
    }

    /// Captures the current desktop screen into an RGBA byte buffer.
    pub fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        let expected_size = (self.width * self.height * 4) as usize;
        if output.len() != expected_size {
            output.resize(expected_size, 0);
        }

        unsafe {
            let blt_res = BitBlt(
                self.hdc_mem,
                0,
                0,
                self.width as i32,
                self.height as i32,
                self.hdc_screen,
                0,
                0,
                SRCCOPY,
            );

            if blt_res.is_err() {
                anyhow::bail!("BitBlt screen capture failed");
            }

            let lines = GetDIBits(
                self.hdc_mem,
                self.h_bitmap,
                0,
                self.height,
                Some(output.as_mut_ptr() as *mut _),
                &mut self.bmi,
                DIB_RGB_COLORS,
            );

            if lines == 0 {
                anyhow::bail!("GetDIBits failed to read screen buffer");
            }

            // Convert BGRA to RGBA in-place (swap R and B channels)
            for pixel in output.chunks_exact_mut(4) {
                let b = pixel[0];
                let r = pixel[2];
                pixel[0] = r;
                pixel[2] = b;
                pixel[3] = 255; // Ensure alpha is fully opaque
            }

            Ok(())
        }
    }
}

impl Drop for ScreenCapture {
    fn drop(&mut self) {
        unsafe {
            if !self.h_bitmap.0.is_null() {
                let _ = DeleteObject(self.h_bitmap);
            }
            if !self.hdc_mem.0.is_null() {
                let _ = DeleteDC(self.hdc_mem);
            }
            if !self.hdc_screen.0.is_null() {
                ReleaseDC(HWND(std::ptr::null_mut()), self.hdc_screen);
            }
        }
    }
}
