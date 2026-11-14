//! Cross-platform Screen Capture — GPU DXGI + GDI on Windows, X11 on Linux.

#[cfg(windows)]
use windows::core::Interface;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
#[cfg(windows)]
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource,
    ID3D11Texture2D, D3D11_CPU_ACCESS_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
#[cfg(windows)]
use windows::Win32::Graphics::Dxgi::{
    IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication, IDXGIResource,
    DXGI_OUTDUPL_FRAME_INFO,
};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
    GetDC, GetDIBits, ReleaseDC, SelectObject, SetStretchBltMode,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, COLORONCOLOR, DIB_RGB_COLORS,
    HBITMAP, HDC, SRCCOPY,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

// ─── Public ScreenCapture type ───────────────────────────────────────────────

pub struct ScreenCapture {
    pub width: u32,
    pub height: u32,
    inner: CaptureBackend,
}

unsafe impl Send for ScreenCapture {}

enum CaptureBackend {
    #[cfg(windows)]
    Dxgi(DxgiCapture),
    #[cfg(windows)]
    Gdi(GdiCapture),
    #[cfg(target_os = "linux")]
    X11(X11Capture),
    #[allow(dead_code)]
    Fallback,
}

impl ScreenCapture {
    pub fn new() -> anyhow::Result<Self> {
        #[cfg(windows)]
        {
            match DxgiCapture::new() {
                Ok(dxgi) => {
                    let (w, h) = (dxgi.width, dxgi.height);
                    tracing::info!("✅ DXGI Desktop Duplication capture initialised ({}×{})", w, h);
                    return Ok(Self { width: w, height: h, inner: CaptureBackend::Dxgi(dxgi) });
                }
                Err(e) => {
                    tracing::warn!("⚠️ DXGI unavailable ({}), falling back to GDI BitBlt", e);
                    let gdi = GdiCapture::new()?;
                    let (w, h) = (gdi.width, gdi.height);
                    return Ok(Self { width: w, height: h, inner: CaptureBackend::Gdi(gdi) });
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let x11 = X11Capture::new()?;
            let (w, h) = (x11.width, x11.height);
            tracing::info!("✅ Linux X11 screen capture initialised ({}×{})", w, h);
            return Ok(Self { width: w, height: h, inner: CaptureBackend::X11(x11) });
        }

        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Err(anyhow::anyhow!("Unsupported operating system for screen capture"))
        }
    }

    pub fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        match &mut self.inner {
            #[cfg(windows)]
            CaptureBackend::Dxgi(d) => d.capture_frame(output),
            #[cfg(windows)]
            CaptureBackend::Gdi(g) => g.capture_frame(output),
            #[cfg(target_os = "linux")]
            CaptureBackend::X11(x) => x.capture_frame(output),
            CaptureBackend::Fallback => Err(anyhow::anyhow!("Capture backend not supported")),
        }
    }
}

// ─── Linux X11 backend ────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
struct X11Capture {
    pub width: u32,
    pub height: u32,
    conn: x11rb::rust_connection::RustConnection,
    root: u32,
    /// Scanline pad bits (from X server setup) — usually 32
    bitmap_pad: usize,
    /// Previous frame hash for change detection (avoid sending identical frames)
    prev_hash: u64,
}

#[cfg(target_os = "linux")]
impl X11Capture {
    fn new() -> anyhow::Result<Self> {
        use x11rb::connection::Connection;
        let (conn, screen_num) = x11rb::connect(None)
            .map_err(|e| anyhow::anyhow!("Failed to connect to X11 display: {}", e))?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;
        let width = screen.width_in_pixels as u32;
        let height = screen.height_in_pixels as u32;

        // Get server bitmap scanline padding requirement (usually 32 bits = 4 bytes aligned)
        let bitmap_pad = conn.setup().bitmap_format_scanline_pad as usize;
        tracing::info!("X11 bitmap_format_scanline_pad = {} bits", bitmap_pad);

        Ok(Self {
            width,
            height,
            conn,
            root,
            bitmap_pad,
            prev_hash: 0,
        })
    }

    fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        use x11rb::protocol::xproto::{ConnectionExt, ImageFormat};

        let w = self.width as usize;
        let h = self.height as usize;
        let expected_rgba = w * h * 4;

        // X11 scanline stride may be padded — calculate actual row stride in bytes
        let pad_bytes = self.bitmap_pad / 8; // bits → bytes (usually 4)
        // For 32bpp ZPixmap: row_stride = ceil(width * 4 / pad_bytes) * pad_bytes
        let row_stride = if pad_bytes > 0 {
            ((w * 4 + pad_bytes - 1) / pad_bytes) * pad_bytes
        } else {
            w * 4
        };

        let reply = self.conn.get_image(
            ImageFormat::Z_PIXMAP,
            self.root,
            0,
            0,
            self.width as u16,
            self.height as u16,
            !0,
        )?.reply()?;

        let data = &reply.data;
        let total_stride_bytes = row_stride * h;

        if data.len() < total_stride_bytes && data.len() < expected_rgba {
            // Partial data — skip frame
            return Err(anyhow::anyhow!("X11 get_image returned incomplete data ({} bytes, expected {})", data.len(), total_stride_bytes));
        }

        // Quick hash on raw data to detect "no change" and avoid wasting CPU + bandwidth
        let frame_hash = fnv_hash(data);
        if frame_hash == self.prev_hash {
            // Identical frame — signal "no desktop changes" just like DXGI does
            return Err(anyhow::anyhow!("No desktop changes"));
        }
        self.prev_hash = frame_hash;

        // Resize output buffer if needed
        if output.len() != expected_rgba {
            output.resize(expected_rgba, 0);
        }

        // X11 ZPixmap on a 24/32-bit display is BGRA (little-endian ARGB32).
        // The tile encoder does BGRA→RGBA swap internally for changed tiles,
        // so we output raw BGRA here (consistent with DXGI/GDI output).
        if row_stride == w * 4 {
            // No padding — fast path: direct copy
            let copy_len = data.len().min(expected_rgba);
            output[..copy_len].copy_from_slice(&data[..copy_len]);
        } else {
            // Padded rows — copy row by row, skipping padding bytes
            let dst_row = w * 4;
            for y in 0..h {
                let src_start = y * row_stride;
                let dst_start = y * dst_row;
                if src_start + dst_row <= data.len() {
                    output[dst_start..dst_start + dst_row]
                        .copy_from_slice(&data[src_start..src_start + dst_row]);
                }
            }
        }

        Ok(())
    }
}

/// Fast FNV-1a 64-bit hash for change detection (much faster than xxHash for this use)
#[cfg(target_os = "linux")]
fn fnv_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 14695981039346656037;
    const FNV_PRIME: u64 = 1099511628211;
    // Sample every 64 bytes for speed — good enough for change detection
    let mut hash = FNV_OFFSET;
    for chunk in data.chunks(64) {
        hash ^= chunk[0] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ─── DXGI Desktop Duplication backend (Windows) ──────────────────────────────

#[cfg(windows)]
struct DxgiCapture {
    width: u32,
    height: u32,
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    dupl: IDXGIOutputDuplication,
    staging: ID3D11Texture2D,
}

#[cfg(windows)]
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

            let device = device.ok_or_else(|| anyhow::anyhow!("Failed to create D3D11 device"))?;
            let context = context.ok_or_else(|| anyhow::anyhow!("Failed to create D3D11 context"))?;

            let dxgi_device: IDXGIDevice = device.cast()?;
            let adapter = dxgi_device.GetAdapter()?;
            let output = adapter.EnumOutputs(0)?;
            let output1: IDXGIOutput1 = output.cast()?;

            let dupl = output1.DuplicateOutput(&device)?;

            let desc = output1.GetDesc()?;
            let width = (desc.DesktopCoordinates.right - desc.DesktopCoordinates.left).unsigned_abs();
            let height = (desc.DesktopCoordinates.bottom - desc.DesktopCoordinates.top).unsigned_abs();

            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: width,
                Height: height,
                MipLevels: 1,
                ArraySize: 1,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
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
            let staging_res: ID3D11Resource = self.staging.cast()?;
            self.context.Map(&staging_res, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;

            let src_pitch = mapped.RowPitch as usize;
            let dst_pitch = self.width as usize * 4;
            let src_ptr = mapped.pData as *const u8;
            let dst_ptr = output.as_mut_ptr();

            // Zero-copy row copy: fast path if row pitch matches desktop width exactly
            if src_pitch == dst_pitch {
                std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, expected);
            } else {
                for y in 0..self.height as usize {
                    let s = src_ptr.add(y * src_pitch);
                    let d = dst_ptr.add(y * dst_pitch);
                    std::ptr::copy_nonoverlapping(s, d, dst_pitch);
                }
            }

            self.context.Unmap(&staging_res, 0);
        }

        Ok(())
    }
}

// ─── GDI BitBlt fallback backend (Windows) ───────────────────────────────────

#[cfg(windows)]
struct GdiCapture {
    width: u32,
    height: u32,
    hdc_screen: HDC,
    hdc_mem: HDC,
    hbm: HBITMAP,
    bmi: BITMAPINFO,
}

#[cfg(windows)]
impl GdiCapture {
    fn new() -> anyhow::Result<Self> {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN) as u32;
            let height = GetSystemMetrics(SM_CYSCREEN) as u32;

            let hdc_screen = GetDC(HWND(std::ptr::null_mut()));
            if hdc_screen.0.is_null() {
                return Err(anyhow::anyhow!("GetDC failed"));
            }

            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbm = CreateCompatibleBitmap(hdc_screen, width as i32, height as i32);
            SelectObject(hdc_mem, hbm);
            SetStretchBltMode(hdc_mem, COLORONCOLOR);

            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = width as i32;
            bmi.bmiHeader.biHeight = -(height as i32); // top-down DIB
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;

            Ok(Self { width, height, hdc_screen, hdc_mem, hbm, bmi })
        }
    }

    fn capture_frame(&mut self, output: &mut Vec<u8>) -> anyhow::Result<()> {
        let expected = (self.width * self.height * 4) as usize;
        if output.len() != expected {
            output.resize(expected, 0);
        }

        unsafe {
            let bit_ok = BitBlt(
                self.hdc_mem,
                0, 0,
                self.width as i32,
                self.height as i32,
                self.hdc_screen,
                0, 0,
                SRCCOPY,
            );
            if bit_ok.is_err() {
                return Err(anyhow::anyhow!("BitBlt failed: {:?}", bit_ok));
            }

            let dib_ok = GetDIBits(
                self.hdc_mem,
                self.hbm,
                0,
                self.height,
                Some(output.as_mut_ptr() as *mut _),
                &mut self.bmi,
                DIB_RGB_COLORS,
            );
            if dib_ok == 0 {
                return Err(anyhow::anyhow!("GetDIBits failed"));
            }
        }

        Ok(())
    }
}

#[cfg(windows)]
impl Drop for GdiCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.hbm);
            let _ = DeleteDC(self.hdc_mem);
            ReleaseDC(HWND(std::ptr::null_mut()), self.hdc_screen);
        }
    }
}
