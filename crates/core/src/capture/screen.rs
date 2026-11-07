//! DXGI Desktop Duplication screen capture — GPU-accelerated, zero-copy path.
//! Falls back to GDI BitBlt if DXGI is unavailable (VMs, RDP, old drivers).
//!
//! AnyDesk RE analysis (Ghidra / REA) confirms: the binary has NO plain-text GDI
//! strings, meaning it exclusively uses DXGI Desktop Duplication (IDXGIOutput1::
//! DuplicateOutput + AcquireNextFrame), which delivers ~0.5 ms capture latency
//! vs ~8 ms for GDI and skips frames with zero CPU when nothing changes.

use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource,
    ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_OUTDUPL_FRAME_INFO,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, StretchBlt, SetStretchBltMode,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, COLORONCOLOR, DIB_RGB_COLORS,
    HBITMAP, HDC, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

// ─── Public ScreenCapture type ───────────────────────────────────────────────

pub struct ScreenCapture {
    pub width: u32,
    pub height: u32,
    inner: CaptureBackend,
}

unsafe impl Send for ScreenCapture {}

enum CaptureBackend {
    Dxgi(DxgiCapture),
    Gdi(GdiCapture),
}

impl ScreenCapture {
    pub fn new() -> anyhow::Result<Self> {
        match DxgiCapture::new() {
            Ok(dxgi) => {
                let (w, h) = (dxgi.width, dxgi.height);
                tracing::info!("✅ DXGI Desktop Duplication capture initialised ({}×{})", w, h);
                Ok(Self { width: w, height: h, inner: CaptureBackend::Dxgi(dxgi) })
            }
            Err(e) => {
                tracing::warn!("⚠️  DXGI unavailable ({}), falling back to GDI BitBlt", e);
                let gdi = GdiCapture::new()?;
                let (w, h) = (gdi.width, gdi.height);
                Ok(Self { width: w, height: h, inner: CaptureBackend::Gdi(gdi) })
            }
        }
    }

    pub fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        match &mut self.inner {
            CaptureBackend::Dxgi(d) => d.capture_frame(output),
            CaptureBackend::Gdi(g) => g.capture_frame(output),
        }
    }
}

// ─── DXGI Desktop Duplication backend ────────────────────────────────────────

struct DxgiCapture {
    width: u32,
    height: u32,
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    dupl: IDXGIOutputDuplication,
    staging: ID3D11Texture2D,
}

impl DxgiCapture {
    fn new() -> anyhow::Result<Self> {
        unsafe {
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                Default::default(),
                None,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
            let device = device.ok_or_else(|| anyhow::anyhow!("No D3D11 device"))?;
            let context = context.ok_or_else(|| anyhow::anyhow!("No D3D11 context"))?;

            let dxgi_device: IDXGIDevice = device.cast()?;
            let adapter = dxgi_device.GetAdapter()?;
            let output = adapter.EnumOutputs(0)?;
            let output1: IDXGIOutput1 = output.cast()?;
            let dupl = output1.DuplicateOutput(&device)?;

            let desc = dupl.GetDesc();
            let width = desc.ModeDesc.Width;
            let height = desc.ModeDesc.Height;

            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: Default::default(),
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: Default::default(),
            };
            let mut staging: Option<ID3D11Texture2D> = None;
            device.CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
            let staging = staging.ok_or_else(|| anyhow::anyhow!("No staging texture"))?;

            Ok(Self { width, height, _device: device, context, dupl, staging })
        }
    }

    fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        let expected = (self.width * self.height * 4) as usize;
        if output.len() != expected {
            output.resize(expected, 0);
        }

        unsafe {
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut desktop_resource: Option<IDXGIResource> = None;
            // 16 ms timeout (synchronous 60 FPS pacing)
            self.dupl.AcquireNextFrame(16, &mut frame_info, &mut desktop_resource)?;

            // If desktop was completely static (0 accumulated frames), release immediately
            if frame_info.AccumulatedFrames == 0 && frame_info.LastPresentTime == 0 {
                self.dupl.ReleaseFrame()?;
                return Err(anyhow::anyhow!("No desktop changes"));
            }

            let desktop_res = desktop_resource
                .ok_or_else(|| anyhow::anyhow!("No desktop resource"))?;
            let desktop_tex: ID3D11Texture2D = desktop_res.cast()?;

            // GPU → staging zero-copy texture blit
            let staging_res: ID3D11Resource = self.staging.cast()?;
            let desktop_res2: ID3D11Resource = desktop_tex.cast()?;
            self.context.CopyResource(&staging_res, &desktop_res2);
            drop(desktop_tex);
            self.dupl.ReleaseFrame()?;

            // Map staging for CPU read
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            self.context.Map(
                &staging_res,
                0,
                D3D11_MAP_READ,
                0,
                Some(&mut mapped),
            )?;

            let src_stride = mapped.RowPitch as usize;
            let dst_stride = self.width as usize * 4;
            let src_ptr = mapped.pData as *const u8;

            // Fast contiguous row memcpy (swapping is deferred strictly to changed tiles)
            for row in 0..self.height as usize {
                let src_row = std::slice::from_raw_parts(
                    src_ptr.add(row * src_stride),
                    dst_stride,
                );
                let dst_start = row * dst_stride;
                output[dst_start..dst_start + dst_stride].copy_from_slice(src_row);
            }

            self.context.Unmap(&staging_res, 0);
            Ok(())
        }
    }
}

// ─── GDI fallback backend ─────────────────────────────────────────────────────

struct GdiCapture {
    width: u32,
    height: u32,
    native_w: u32,
    native_h: u32,
    hdc_screen: HDC,
    hdc_mem: HDC,
    h_bitmap: HBITMAP,
    bmi: BITMAPINFO,
}

impl GdiCapture {
    fn new() -> anyhow::Result<Self> {
        unsafe {
            let native_w = GetSystemMetrics(SM_CXSCREEN) as u32;
            let native_h = GetSystemMetrics(SM_CYSCREEN) as u32;
            // Downscale to 720p on 1080p+ screens
            let (target_w, target_h) = if native_w >= 1920 { (1280, 720) } else { (native_w, native_h) };

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            if hdc_screen.0.is_null() { anyhow::bail!("Failed to get desktop DC"); }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let h_bitmap = CreateCompatibleBitmap(hdc_screen, target_w as i32, target_h as i32);
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: target_w as i32,
                    biHeight: -(target_h as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                bmiColors: [Default::default(); 1],
            };
            SelectObject(hdc_mem, h_bitmap);
            Ok(Self { width: target_w, height: target_h, native_w, native_h,
                      hdc_screen, hdc_mem, h_bitmap, bmi })
        }
    }

    fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        let expected = (self.width * self.height * 4) as usize;
        if output.len() != expected { output.resize(expected, 0); }
        unsafe {
            let ok = if self.width == self.native_w && self.height == self.native_h {
                BitBlt(self.hdc_mem, 0, 0, self.width as i32, self.height as i32,
                       self.hdc_screen, 0, 0, SRCCOPY).is_ok()
            } else {
                let _ = SetStretchBltMode(self.hdc_mem, COLORONCOLOR);
                // StretchBlt returns BOOL (not Result) in this windows version
                let result = StretchBlt(self.hdc_mem, 0, 0, self.width as i32, self.height as i32,
                           self.hdc_screen, 0, 0, self.native_w as i32, self.native_h as i32,
                           SRCCOPY);
                result.as_bool()
            };
            if !ok { anyhow::bail!("GDI BLT failed"); }
            let lines = GetDIBits(self.hdc_mem, self.h_bitmap, 0, self.height,
                                   Some(output.as_mut_ptr() as *mut _), &mut self.bmi, DIB_RGB_COLORS);
            if lines == 0 { anyhow::bail!("GetDIBits failed"); }
            for px in output.chunks_exact_mut(4) { px.swap(0, 2); px[3] = 255; }
            Ok(())
        }
    }
}

impl Drop for GdiCapture {
    fn drop(&mut self) {
        unsafe {
            if !self.h_bitmap.0.is_null() { let _ = DeleteObject(self.h_bitmap); }
            if !self.hdc_mem.0.is_null() { let _ = DeleteDC(self.hdc_mem); }
            if !self.hdc_screen.0.is_null() { ReleaseDC(HWND(std::ptr::null_mut()), self.hdc_screen); }
        }
    }
}
