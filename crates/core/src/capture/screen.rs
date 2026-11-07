//! Screen capture implementation for Windows.
//! Provides fast desktop frame buffer acquisition.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BitBlt, StretchBlt, SetStretchBltMode, COLORONCOLOR, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
    GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP, HDC, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

pub struct ScreenCapture {
    pub width: u32,
    pub height: u32,
    pub native_w: u32,
    pub native_h: u32,
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
            // Capture at target resolution for high-speed streaming
            let native_w = GetSystemMetrics(SM_CXSCREEN) as u32;
            let native_h = GetSystemMetrics(SM_CYSCREEN) as u32;

            // Target 720p standard streaming resolution (1280x720) or native if smaller for 60+ FPS
            let (target_w, target_h) = if native_w >= 1920 {
                (1280, 720)
            } else {
                (native_w, native_h)
            };

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            if hdc_screen.0.is_null() {
                anyhow::bail!("Failed to get desktop device context");
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let h_bitmap = CreateCompatibleBitmap(hdc_screen, target_w as i32, target_h as i32);

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: target_w as i32,
                    biHeight: -(target_h as i32), // Top-down DIB
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
            let _ = SetStretchBltMode(hdc_mem, COLORONCOLOR);

            Ok(Self {
                width: target_w,
                height: target_h,
                native_w,
                native_h,
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
            let blt_res = if self.width == self.native_w && self.height == self.native_h {
                BitBlt(
                    self.hdc_mem,
                    0,
                    0,
                    self.width as i32,
                    self.height as i32,
                    self.hdc_screen,
                    0,
                    0,
                    SRCCOPY,
                )
            } else {
                StretchBlt(
                    self.hdc_mem,
                    0,
                    0,
                    self.width as i32,
                    self.height as i32,
                    self.hdc_screen,
                    0,
                    0,
                    self.native_w as i32,
                    self.native_h as i32,
                    SRCCOPY,
                ).ok()
            };

            if blt_res.is_err() {
                anyhow::bail!("Screen capture BLT failed");
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
