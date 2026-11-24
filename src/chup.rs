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
        Some(Anh {
            w,
            h,
            x0: self.x0 + x0 as i32,
            y0: self.y0 + y0 as i32,
            bgra,
        })
    }

    /// Lọc chữ trắng (giống app cũ): độ sáng ≥ ngưỡng → chữ ĐEN, còn lại nền TRẮNG.
    pub fn loc_trang(&mut self, nguong: u8) {
        for p in self.bgra.chunks_exact_mut(4) {
            let xam = (p[2] as u32 * 299 + p[1] as u32 * 587 + p[0] as u32 * 114) / 1000;
            let v = if xam >= nguong as u32 { 0 } else { 255 };
            p[0] = v;
            p[1] = v;
            p[2] = v;
        }
    }

    /// Đổi cỡ ảnh mịn (nội suy song tuyến) – OCR đọc chữ nhỏ tốt hơn.
    pub fn doi_co(&self, w: u32, h: u32) -> Anh {
        let (w, h) = (w.max(1), h.max(1));
        let mut bgra = vec![0u8; (w * h * 4) as usize];
        let sx = self.w as f32 / w as f32;
        let sy = self.h as f32 / h as f32;
        for y in 0..h {
            let fy = ((y as f32 + 0.5) * sy - 0.5).max(0.0);
            let y0 = (fy as u32).min(self.h - 1);
            let y1 = (y0 + 1).min(self.h - 1);
            let ty = fy - y0 as f32;
            for x in 0..w {
                let fx = ((x as f32 + 0.5) * sx - 0.5).max(0.0);
                let x0 = (fx as u32).min(self.w - 1);
                let x1 = (x0 + 1).min(self.w - 1);
                let tx = fx - x0 as f32;
                let d = ((y * w + x) * 4) as usize;
                for k in 0..4 {
                    let p =
                        |xx: u32, yy: u32| self.bgra[((yy * self.w + xx) * 4) as usize + k] as f32;
                    let tren = p(x0, y0) * (1.0 - tx) + p(x1, y0) * tx;
                    let duoi = p(x0, y1) * (1.0 - tx) + p(x1, y1) * tx;
                    bgra[d + k] = (tren * (1.0 - ty) + duoi * ty).round() as u8;
                }
            }
        }
        Anh {
            w,
            h,
            x0: self.x0,
            y0: self.y0,
            bgra,
        }
    }

    /// Dấu vân tay ảnh (96×24 xám) – ảnh không đổi thì khỏi đọc chữ lại.
    pub fn van_tay(&self) -> Vec<u8> {
        let (w, h) = (96u32, 24u32);
        let mut ra = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                let sx = (x * self.w / w).min(self.w - 1);
                let sy = (y * self.h / h).min(self.h - 1);
                let i = ((sy * self.w + sx) * 4) as usize;
                ra.push(
                    ((self.bgra[i + 2] as u32 * 299
                        + self.bgra[i + 1] as u32 * 587
                        + self.bgra[i] as u32 * 114)
                        / 1000) as u8,
                );
            }
        }
        ra
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
            Ok(Self {
                dev,
                ctx,
                dup,
                tam: None,
                w: 0,
                h: 0,
            })
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
        if self.w == 0 {
            return ChupGdi.kich_thuoc();
        }
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
                    return Err(anyhow!(
                        "DXGI mất quyền chụp (game đổi chế độ hiển thị / tắt)"
                    ));
                }
                Err(e) => return Err(e.into()),
            }
            // Chỉ chuột di chuyển (không có hình mới) → coi như không đổi
            if info.LastPresentTime == 0 && self.tam.is_some() {
                let _ = self.dup.ReleaseFrame();
                return Ok(false);
            }
            let kq = (|| -> Result<()> {
                let tex: ID3D11Texture2D =
                    tai_nguyen.ok_or_else(|| anyhow!("khung rỗng"))?.cast()?;
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
        // Chưa nhận được khung hình nào (màn hình đứng yên từ lúc tạo) → chụp tạm bằng GDI
        let Some(tam) = self.tam.as_ref() else {
            return ChupGdi.lay(v);
        };
        let (x0, y0, w, h) = kep(v, self.w, self.h)?;
        unsafe {
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx
                .Map(tam, 0, D3D11_MAP_READ, 0, Some(&mut map))
                .ok()?;
            let goc = map.pData as *const u8;
            let mut bgra = Vec::with_capacity((w * h * 4) as usize);
            for y in y0..y0 + h {
                let dau = goc.add(y as usize * map.RowPitch as usize + x0 as usize * 4);
                bgra.extend_from_slice(std::slice::from_raw_parts(dau, (w * 4) as usize));
            }
            self.ctx.Unmap(tam, 0);
            Some(Anh {
                w,
                h,
                x0: x0 as i32,
                y0: y0 as i32,
                bgra,
            })
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
        unsafe {
            (
                GetSystemMetrics(SM_CXSCREEN) as u32,
                GetSystemMetrics(SM_CYSCREEN) as u32,
            )
        }
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
            let ok = BitBlt(
                dc,
                0,
                0,
                wi,
                hi,
                Some(man),
                x0 as i32,
                y0 as i32,
                SRCCOPY | CAPTUREBLT,
            );
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = wi;
            info.bmiHeader.biHeight = -hi;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut bgra = vec![0u8; (w * h * 4) as usize];
            SelectObject(dc, cu);
            GetDIBits(
                dc,
                bmp,
                0,
                h,
                Some(bgra.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(dc);
            ReleaseDC(None, man);
            ok.ok()?;
            Some(Anh {
                w,
                h,
                x0: x0 as i32,
                y0: y0 as i32,
                bgra,
            })
        }
    }
}
