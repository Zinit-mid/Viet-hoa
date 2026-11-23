//! Chụp màn hình: DXGI Desktop Duplication (chụp đúng game Borderless, bỏ qua
//! cửa sổ có WDA_EXCLUDEFROMCAPTURE) → dự phòng GDI.
//! Chỉ đọc phần vùng cần dùng; màn hình không đổi thì báo để bỏ qua OCR.

use anyhow::{anyhow, Result};
use windows::core::Interface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

/// Ảnh BGRA, hàng trên cùng trước. `x0, y0` = vị trí góc trái trên màn hình.
#[derive(Clone, Default)]
pub struct Anh {
    pub w: u32,
    pub h: u32,
    pub x0: i32,
    pub y0: i32,
    pub bgra: Vec<u8>,
}

impl Anh {
    #[inline]
    pub fn diem(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.bgra[i + 2], self.bgra[i + 1], self.bgra[i]]
    }

    /// Cắt [x, y, w, h] theo toạ độ trong ảnh.
    pub fn cat(&self, v: [i32; 4]) -> Option<Anh> {
        let x0 = v[0].clamp(0, self.w as i32) as u32;
        let y0 = v[1].clamp(0, self.h as i32) as u32;
        let x1 = (v[0] + v[2]).clamp(0, self.w as i32) as u32;
        let y1 = (v[1] + v[3]).clamp(0, self.h as i32) as u32;
        if x1 <= x0 + 4 || y1 <= y0 + 4 {
            return None;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        let mut bgra = Vec::with_capacity((w * h * 4) as usize);
        for y in y0..y1 {
            let d = ((y * self.w + x0) * 4) as usize;
            bgra.extend_from_slice(&self.bgra[d..d + (w * 4) as usize]);
        }
        Some(Anh { w, h, x0: self.x0 + x0 as i32, y0: self.y0 + y0 as i32, bgra })
    }

    /// Phóng to nguyên lần (giúp OCR đọc chữ nhỏ).
    pub fn phong(&self, k: u32) -> Anh {
        if k <= 1 {
            return self.clone();
        }
        let (w, h) = (self.w * k, self.h * k);
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            let hang_nguon = ((y / k) * self.w * 4) as usize;
            let hang_dich = (y * w * 4) as usize;
            for x in 0..w {
                let s = hang_nguon + ((x / k) * 4) as usize;
                let d = hang_dich + (x * 4) as usize;
                bgra[d..d + 4].copy_from_slice(&self.bgra[s..s + 4]);
            }
        }
        Anh { w, h, x0: self.x0, y0: self.y0, bgra }
    }

    /// Lọc chữ trắng (phụ đề): điểm sáng gần trắng giữ, còn lại thành đen.
    pub fn loc_trang(&mut self) {
        for p in self.bgra.chunks_exact_mut(4) {
            let mn = p[0].min(p[1]).min(p[2]);
            let mx = p[0].max(p[1]).max(p[2]);
            let v = if mn > 165 && mx - mn < 45 { 255 } else { 0 };
            p[0] = v;
            p[1] = v;
            p[2] = v;
        }
    }
}

pub trait MayChup {
    /// Lấy khung hình mới. Trả false nếu màn hình không đổi kể từ lần trước.
    fn cap_nhat(&mut self) -> Result<bool>;
    /// Đọc vùng [x, y, w, h] (toạ độ màn hình) của khung hình hiện có.
    fn lay(&mut self, v: [i32; 4]) -> Option<Anh>;
    fn kich_thuoc(&self) -> (u32, u32);
    fn ten(&self) -> &'static str;
}

fn kep(v: [i32; 4], w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let x0 = v[0].clamp(0, w as i32) as u32;
    let y0 = v[1].clamp(0, h as i32) as u32;
    let x1 = (v[0] + v[2]).clamp(0, w as i32) as u32;
    let y1 = (v[1] + v[3]).clamp(0, h as i32) as u32;
    (x1 > x0 + 4 && y1 > y0 + 4).then_some((x0, y0, x1 - x0, y1 - y0))
}

// ---------------- DXGI ----------------

pub struct ChupDxgi {
    dev: ID3D11Device,
    ctx: ID3D11DeviceContext,
    dup: IDXGIOutputDuplication,
    /// Bản sao khung hình cuối cùng trên GPU (đọc được bằng CPU)
    tam: Option<ID3D11Texture2D>,
    w: u32,
    h: u32,
}

// Đối tượng COM chỉ dùng trong một luồng dịch.
unsafe impl Send for ChupDxgi {}

impl ChupDxgi {
    pub fn moi() -> Result<Self> {
        unsafe {
            let mut dev = None;
            let mut ctx = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut dev),
                None,
                Some(&mut ctx),
            )?;
            let dev = dev.ok_or_else(|| anyhow!("không tạo được D3D11"))?;
            let ctx = ctx.ok_or_else(|| anyhow!("không tạo được D3D11"))?;
            let dup = Self::nhan_ban(&dev)?;
            Ok(Self { dev, ctx, dup, tam: None, w: 0, h: 0 })
        }
    }

    unsafe fn nhan_ban(dev: &ID3D11Device) -> Result<IDXGIOutputDuplication> {
        let dxgi: IDXGIDevice = dev.cast()?;
        let adapter = dxgi.GetAdapter()?;
        let out: IDXGIOutput1 = adapter.EnumOutputs(0)?.cast()?;
        Ok(out.DuplicateOutput(dev)?)
    }
}

impl MayChup for ChupDxgi {
    fn ten(&self) -> &'static str {
        "DXGI"
    }

    fn kich_thuoc(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    fn cap_nhat(&mut self) -> Result<bool> {
        unsafe {
            let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut tai_nguyen: Option<IDXGIResource> = None;
            match self.dup.AcquireNextFrame(100, &mut info, &mut tai_nguyen) {
                Ok(()) => {}
                Err(e) if e.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(false),
                Err(e) if e.code() == DXGI_ERROR_ACCESS_LOST => {
                    // Không tự tạo lại ở đây: bộ quản lý sẽ bỏ hẳn bộ chụp này và chờ rồi mới tạo mới
                    return Err(anyhow!("DXGI mất quyền chụp (game đổi chế độ hiển thị / tắt)"));
                }
                Err(e) => return Err(e.into()),
            }
            // Chỉ chuột di chuyển (không có hình mới) → coi như không đổi
            if info.LastPresentTime == 0 && self.tam.is_some() {
                let _ = self.dup.ReleaseFrame();
                return Ok(false);
            }
            let kq = (|| -> Result<()> {
                let tex: ID3D11Texture2D = tai_nguyen.ok_or_else(|| anyhow!("khung rỗng"))?.cast()?;
                let mut mo_ta = D3D11_TEXTURE2D_DESC::default();
                tex.GetDesc(&mut mo_ta);
                if self.tam.is_none() || self.w != mo_ta.Width || self.h != mo_ta.Height {
                    let mut d = mo_ta;
                    d.Usage = D3D11_USAGE_STAGING;
                    d.BindFlags = 0;
                    d.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
                    d.MiscFlags = 0;
                    d.MipLevels = 1;
                    d.ArraySize = 1;
                    let mut t = None;
                    self.dev.CreateTexture2D(&d, None, Some(&mut t))?;
                    self.tam = t;
                    self.w = mo_ta.Width;
                    self.h = mo_ta.Height;
                }
                // Chép trên GPU (rất nhanh), CPU chỉ đọc phần cần khi gọi lay()
                self.ctx.CopyResource(self.tam.as_ref().unwrap(), &tex);
                Ok(())
            })();
            let _ = self.dup.ReleaseFrame();
            kq?;
            Ok(true)
        }
    }

    fn lay(&mut self, v: [i32; 4]) -> Option<Anh> {
        let tam = self.tam.as_ref()?;
        let (x0, y0, w, h) = kep(v, self.w, self.h)?;
        unsafe {
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx.Map(tam, 0, D3D11_MAP_READ, 0, Some(&mut map)).ok()?;
            let goc = map.pData as *const u8;
            let mut bgra = Vec::with_capacity((w * h * 4) as usize);
            for y in y0..y0 + h {
                let dau = goc.add(y as usize * map.RowPitch as usize + x0 as usize * 4);
                bgra.extend_from_slice(std::slice::from_raw_parts(dau, (w * 4) as usize));
            }
            self.ctx.Unmap(tam, 0);
            Some(Anh { w, h, x0: x0 as i32, y0: y0 as i32, bgra })
        }
    }
}

// ---------------- GDI ----------------

#[derive(Default)]
pub struct ChupGdi;

impl MayChup for ChupGdi {
    fn ten(&self) -> &'static str {
        "GDI"
    }

    fn kich_thuoc(&self) -> (u32, u32) {
        unsafe { (GetSystemMetrics(SM_CXSCREEN) as u32, GetSystemMetrics(SM_CYSCREEN) as u32) }
    }

    fn cap_nhat(&mut self) -> Result<bool> {
        Ok(true)
    }

    fn lay(&mut self, v: [i32; 4]) -> Option<Anh> {
        let (sw, sh) = self.kich_thuoc();
        let (x0, y0, w, h) = kep(v, sw, sh)?;
        let (wi, hi) = (w as i32, h as i32);
        unsafe {
            let man = GetDC(None);
            let dc = CreateCompatibleDC(Some(man));
            let bmp = CreateCompatibleBitmap(man, wi, hi);
            let cu = SelectObject(dc, bmp.into());
            let ok = BitBlt(dc, 0, 0, wi, hi, Some(man), x0 as i32, y0 as i32, SRCCOPY | CAPTUREBLT);
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = wi;
            info.bmiHeader.biHeight = -hi;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            SelectObject(dc, cu);
            GetDIBits(dc, bmp, 0, h, Some(bgra.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(dc);
            ReleaseDC(None, man);
            ok.ok()?;
            Some(Anh { w, h, x0: x0 as i32, y0: y0 as i32, bgra })
        }
    }
}
