//! Khung bản dịch của từng vùng (giống KhungBanDich của app cũ).
//! - Kiểu "phụ đề": chữ trắng viền đen, nền trong suốt.  - Kiểu "khung": hộp nền tối.
//! - Che chữ gốc: ô cùng màu nền đè lên chữ tiếng Anh, chữ Việt cùng màu chữ gốc.
//! - Chế độ chỉnh (F11): kéo để dời, kéo góc ◢ đổi độ rộng, lăn chuột đổi cỡ chữ.
//! Click xuyên khi không chỉnh; không bị chụp (WDA_EXCLUDEFROMCAPTURE).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows::core::w;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Kieu {
    /// Chữ viền, nền trong suốt
    PhuDe,
    /// Hộp nền tối
    Hop,
    /// Che chữ gốc: (x, y, w, h, màu nền, màu chữ, cao 1 dòng)
    Che {
        hop: [i32; 4],
        nen: [u8; 3],
        chu: [u8; 3],
        cao: i32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct KhungHien {
    pub id: String,
    pub ten: String,
    pub chu: String,
    /// Khung [x, y, w]
    pub khung: [i32; 3],
    pub co_chu: i32,
    pub kieu: Kieu,
    pub can_giua: bool,
    pub do_dam: f32,
    pub vien: i32,
}

/// Người dùng vừa kéo / đổi cỡ khung (chế độ chỉnh).
#[derive(Debug, Clone)]
pub struct KhungDoi {
    pub id: String,
    pub khung: [i32; 3],
    pub co_chu: i32,
}

const WM_CAP_NHAT: u32 = WM_APP + 11;
const MAU_KHOA: COLORREF = COLORREF(0x0001_0101);
const LE: i32 = 10;

static NOI_DUNG: Mutex<Vec<KhungHien>> = Mutex::new(Vec::new());
static DANG_CHINH: AtomicBool = AtomicBool::new(false);
static HWND_DK: AtomicIsize = AtomicIsize::new(0);
static GUI: OnceLock<Mutex<Sender<KhungDoi>>> = OnceLock::new();

struct O {
    hwnd: HWND,
    hien: Option<KhungHien>,
    /// Đang kéo: (chuột x, y, khung x, y, w, đổi độ rộng?)
    keo: Option<(i32, i32, i32, i32, i32, bool)>,
    /// Vị trí / cỡ đang dùng (có thể khác cấu hình khi đang kéo)
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    co: i32,
}

thread_local! {
    static CAC_O: RefCell<HashMap<String, O>> = RefCell::new(HashMap::new());
    static FONT: RefCell<HashMap<i32, HFONT>> = RefCell::new(HashMap::new());
}

pub fn dang_chinh() -> bool {
    DANG_CHINH.load(Ordering::Relaxed)
}

/// Đặt nội dung tất cả khung (gọi từ luồng nào cũng được).
pub fn dat(ds: Vec<KhungHien>) {
    {
        let mut nd = NOI_DUNG.lock().unwrap_or_else(|e| e.into_inner());
        if *nd == ds {
            return;
        }
        *nd = ds;
    }
    bao_cap_nhat();
}

/// Bật / tắt chế độ chỉnh khung bằng chuột (F11).
pub fn chinh(bat: bool) {
    DANG_CHINH.store(bat, Ordering::Relaxed);
    bao_cap_nhat();
}

fn bao_cap_nhat() {
    let h = HWND_DK.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_CAP_NHAT, WPARAM(0), LPARAM(0));
        }
    }
}

pub fn khoi_dong(gui: Sender<KhungDoi>) {
    let _ = GUI.set(Mutex::new(gui));
    std::thread::spawn(|| unsafe { luong() });
}

fn hinst() -> HINSTANCE {
    unsafe { GetModuleHandleW(None).unwrap_or_default().into() }
}

fn rgb(c: [u8; 3]) -> COLORREF {
    COLORREF(c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16)
}

unsafe fn luong() {
    let h = hinst();
    RegisterClassW(&WNDCLASSW {
        lpfnWndProc: Some(thu_dk),
        hInstance: h,
        lpszClassName: w!("VietHoaKhungDK"),
        ..Default::default()
    });
    RegisterClassW(&WNDCLASSW {
        lpfnWndProc: Some(thu_khung),
        hInstance: h,
        lpszClassName: w!("VietHoaKhungVung"),
        hCursor: LoadCursorW(None, IDC_SIZEALL).unwrap_or_default(),
        ..Default::default()
    });
    let Ok(dk) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("VietHoaKhungDK"),
        w!(""),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        Some(h),
        None,
    ) else {
        return;
    };
    HWND_DK.store(dk.0 as isize, Ordering::Relaxed);
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }
}

unsafe extern "system" fn thu_dk(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CAP_NHAT {
        cap_nhat();
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

unsafe fn font(co: i32, dam: bool) -> HFONT {
    let khoa = co * 2 + dam as i32;
    FONT.with(|f| {
        *f.borrow_mut().entry(khoa).or_insert_with(|| {
            CreateFontW(
                -co,
                0,
                0,
                0,
                if dam {
                    FW_BOLD.0 as i32
                } else {
                    FW_SEMIBOLD.0 as i32
                },
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                ANTIALIASED_QUALITY,
                (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
                w!("Be Vietnam Pro"),
            )
        })
    })
}

/// Đo chữ với bề rộng cho trước → (rộng, cao).
unsafe fn do_chu(chu: &str, co: i32, rong: i32) -> (i32, i32) {
    let mut s: Vec<u16> = chu.encode_utf16().collect();
    if s.is_empty() {
        return (0, co);
    }
    let dc = CreateCompatibleDC(None);
    let cu = SelectObject(dc, font(co, true).into());
    let mut r = RECT {
        left: 0,
        top: 0,
        right: rong.max(40),
        bottom: 0,
    };
    DrawTextW(dc, &mut s, &mut r, DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX);
    SelectObject(dc, cu);
    let _ = DeleteDC(dc);
    (r.right, r.bottom)
}

unsafe fn tao_cua_so() -> Option<HWND> {
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        w!("VietHoaKhungVung"),
        w!("Việt Hóa – khung dịch"),
        WS_POPUP,
        0,
        0,
        10,
        10,
        None,
        None,
        Some(hinst()),
        None,
    )
    .ok()?;
    let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
    Some(hwnd)
}

/// Kích thước khung theo nội dung (giống _ve của app cũ).
unsafe fn tinh_co(
    k: &KhungHien,
    chinh: bool,
    x: i32,
    y: i32,
    w: i32,
    co: i32,
) -> (i32, i32, i32, i32, i32) {
    if let (Kieu::Che { hop, cao, .. }, false) = (&k.kieu, chinh) {
        // giống ve_chu_vua_o: cỡ từ 0,9 × cao dòng, thu nhỏ ×0,9 tới khi vừa; vẫn dài thì kéo ô xuống
        let [cx, cy, cw, ch] = *hop;
        let mut c = ((*cao as f32) * 0.9) as i32;
        c = c.max(10);
        loop {
            let (_, hh) = do_chu(&k.chu, c, cw - 8);
            if hh as f32 <= (ch - 6) as f32 * 1.08 || c <= 10 {
                return (cx, cy, cw, ch.max(hh + 6), c);
            }
            c = ((c as f32 * 0.9) as i32).max(10);
        }
    }
    let tren = if chinh { 26 } else { LE };
    let chu = if chinh && k.chu.trim().is_empty() {
        GOI_Y
    } else {
        k.chu.as_str()
    };
    let (_, hh) = do_chu(chu, co, (w - 2 * LE).max(60));
    (x, y, w, (hh + tren + LE).max(30), co)
}

const GOI_Y: &str = "Kéo để di chuyển • kéo góc ◢ đổi độ rộng • lăn chuột đổi cỡ chữ";

unsafe fn cap_nhat() {
    let ds = NOI_DUNG.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let chinh = dang_chinh();
    CAC_O.with(|o| {
        let mut o = o.borrow_mut();
        // khung không còn → ẩn
        for (id, x) in o.iter_mut() {
            if !ds.iter().any(|k| &k.id == id) && x.hien.is_some() {
                x.hien = None;
                let _ = ShowWindow(x.hwnd, SW_HIDE);
            }
        }
        for k in ds {
            let hien = chinh || !k.chu.trim().is_empty();
            if !o.contains_key(&k.id) {
                let Some(h) = tao_cua_so() else { continue };
                o.insert(
                    k.id.clone(),
                    O {
                        hwnd: h,
                        hien: None,
                        keo: None,
                        x: 0,
                        y: 0,
                        w: 0,
                        h: 0,
                        co: 0,
                    },
                );
            }
            let x = o.get_mut(&k.id).unwrap();
            if !hien {
                if x.hien.is_some() {
                    x.hien = None;
                    let _ = ShowWindow(x.hwnd, SW_HIDE);
                }
                continue;
            }
            if x.keo.is_some() {
                // đang kéo: giữ vị trí người dùng, chỉ đổi chữ
                x.hien = Some(KhungHien {
                    khung: [x.x, x.y, x.w],
                    co_chu: x.co,
                    ..k
                });
            } else {
                x.x = k.khung[0];
                x.y = k.khung[1];
                x.w = k.khung[2].max(150);
                x.co = k.co_chu;
                x.hien = Some(k);
            }
            ve_lai(x, chinh);
        }
    });
}

unsafe fn ve_lai(o: &mut O, chinh: bool) {
    let Some(k) = o.hien.clone() else { return };
    let (x, y, w, h, _) = tinh_co(&k, chinh, o.x, o.y, o.w, o.co);
    o.h = h;
    let mut kieu = GetWindowLongW(o.hwnd, GWL_EXSTYLE) as u32;
    if chinh {
        kieu &= !WS_EX_TRANSPARENT.0;
    } else {
        kieu |= WS_EX_TRANSPARENT.0;
    }
    SetWindowLongW(o.hwnd, GWL_EXSTYLE, kieu as i32);
    let (mau, alpha, co) = match (&k.kieu, chinh) {
        (_, true) => (LWA_ALPHA, 242u8, LWA_ALPHA),
        (Kieu::PhuDe, _) => (
            LWA_COLORKEY | LWA_ALPHA,
            (k.do_dam.clamp(0.2, 1.0) * 255.0) as u8,
            LWA_ALPHA,
        ),
        (Kieu::Hop, _) => (
            LWA_ALPHA,
            (k.do_dam.clamp(0.2, 1.0) * 255.0) as u8,
            LWA_ALPHA,
        ),
        (Kieu::Che { .. }, _) => (LWA_ALPHA, 255, LWA_ALPHA),
    };
    let _ = co;
    let _ = SetLayeredWindowAttributes(o.hwnd, MAU_KHOA, alpha, mau);
    let _ = SetWindowPos(
        o.hwnd,
        Some(HWND_TOPMOST),
        x,
        y,
        w,
        h,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    let _ = InvalidateRect(Some(o.hwnd), None, false);
}

unsafe fn ve(hwnd: HWND) {
    let mut ps = PAINTSTRUCT::default();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let chinh = dang_chinh();
    let k = CAC_O.with(|o| {
        o.borrow()
            .values()
            .find(|x| x.hwnd == hwnd)
            .and_then(|x| x.hien.clone().map(|k| (k, x.co)))
    });
    if let Some((k, co)) = k {
        // vẽ vào bộ đệm rồi chép một lần (không nháy)
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, rc.right, rc.bottom);
        let cu_bmp = SelectObject(mem, bmp.into());
        SetBkMode(mem, TRANSPARENT);
        let nen = match (&k.kieu, chinh) {
            (_, true) => [0x2b, 0x2b, 0x33],
            (Kieu::PhuDe, _) => [1, 1, 1],
            (Kieu::Hop, _) => [0x10, 0x10, 0x14],
            (Kieu::Che { nen, .. }, _) => {
                if *nen == [1, 1, 1] {
                    [0, 0, 0]
                } else {
                    *nen
                }
            }
        };
        let co_nen = CreateSolidBrush(rgb(nen));
        FillRect(mem, &rc, co_nen);
        let _ = DeleteObject(co_nen.into());

        let mut tren = LE;
        if chinh {
            let cu = SelectObject(mem, font(12, true).into());
            SetTextColor(mem, rgb([0xff, 0xcc, 0x33]));
            let mut t: Vec<u16> = format!("■ {}", k.ten).encode_utf16().collect();
            let mut r = RECT {
                left: LE,
                top: 6,
                right: rc.right - LE,
                bottom: 24,
            };
            DrawTextW(mem, &mut t, &mut r, DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(mem, cu);
            tren = 26;
        }
        let chu = if chinh && k.chu.trim().is_empty() {
            GOI_Y.to_string()
        } else {
            k.chu.clone()
        };
        let mut s: Vec<u16> = chu.encode_utf16().collect();
        let can = if k.can_giua { DT_CENTER } else { DT_LEFT };
        match (&k.kieu, chinh) {
            (Kieu::Che { chu: mau, .. }, false) => {
                let (_, _, _, _, c) = tinh_co(&k, false, 0, 0, 0, 0);
                let cu = SelectObject(mem, font(c, true).into());
                SetTextColor(mem, rgb(*mau));
                let mut r = RECT {
                    left: 4,
                    top: 3,
                    right: rc.right - 4,
                    bottom: rc.bottom - 3,
                };
                DrawTextW(mem, &mut s, &mut r, DT_WORDBREAK | DT_NOPREFIX | can);
                SelectObject(mem, cu);
            }
            _ => {
                let cu = SelectObject(mem, font(co, true).into());
                let r0 = RECT {
                    left: LE,
                    top: tren,
                    right: rc.right - LE,
                    bottom: rc.bottom,
                };
                // viền chữ (kiểu phụ đề)
                let vien = if matches!(k.kieu, Kieu::PhuDe) && !chinh {
                    k.vien + (co >= 26 && k.vien > 0) as i32
                } else {
                    0
                };
                if vien > 0 {
                    SetTextColor(mem, COLORREF(0));
                    for dx in -vien..=vien {
                        for dy in -vien..=vien {
                            if (dx != 0 || dy != 0) && dx * dx + dy * dy <= vien * vien + 1 {
                                let mut r = RECT {
                                    left: r0.left + dx,
                                    top: r0.top + dy,
                                    right: r0.right + dx,
                                    bottom: r0.bottom + dy,
                                };
                                DrawTextW(mem, &mut s, &mut r, DT_WORDBREAK | DT_NOPREFIX | can);
                            }
                        }
                    }
                }
                SetTextColor(mem, COLORREF(0x00FF_FFFF));
                let mut r = r0;
                DrawTextW(mem, &mut s, &mut r, DT_WORDBREAK | DT_NOPREFIX | can);
                SelectObject(mem, cu);
            }
        }
        if chinh {
            // viền nét đứt vàng + tam giác ◢ ở góc
            let but = CreatePen(PS_DASH, 1, rgb([0xff, 0xcc, 0x33]));
            let cu_but = SelectObject(mem, but.into());
            let cu_choi = SelectObject(mem, GetStockObject(NULL_BRUSH));
            let _ = Rectangle(mem, 1, 1, rc.right - 1, rc.bottom - 1);
            let _ = Rectangle(mem, 2, 2, rc.right - 2, rc.bottom - 2);
            SelectObject(mem, cu_choi);
            SelectObject(mem, cu_but);
            let _ = DeleteObject(but.into());
            let vang = CreateSolidBrush(rgb([0xff, 0xcc, 0x33]));
            let cu_choi = SelectObject(mem, vang.into());
            let pt = [
                POINT {
                    x: rc.right - 2,
                    y: rc.bottom - 22,
                },
                POINT {
                    x: rc.right - 2,
                    y: rc.bottom - 2,
                },
                POINT {
                    x: rc.right - 22,
                    y: rc.bottom - 2,
                },
            ];
            let _ = Polygon(mem, &pt);
            SelectObject(mem, cu_choi);
            let _ = DeleteObject(vang.into());
        }
        let _ = BitBlt(hdc, 0, 0, rc.right, rc.bottom, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, cu_bmp);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
    }
    let _ = EndPaint(hwnd, &ps);
}

fn bao_doi(o: &O) {
    if let (Some(k), Some(g)) = (&o.hien, GUI.get()) {
        if let Ok(g) = g.lock() {
            let _ = g.send(KhungDoi {
                id: k.id.clone(),
                khung: [o.x, o.y, o.w],
                co_chu: o.co,
            });
        }
    }
}

unsafe extern "system" fn thu_khung(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let diem = || {
        let mut p = POINT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p);
        p
    };
    match msg {
        WM_PAINT => {
            ve(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_LBUTTONDOWN if dang_chinh() => {
            let (cx, cy) = (
                (lp.0 & 0xFFFF) as i16 as i32,
                ((lp.0 >> 16) & 0xFFFF) as i16 as i32,
            );
            let p = diem();
            CAC_O.with(|o| {
                if let Some(x) = o.borrow_mut().values_mut().find(|x| x.hwnd == hwnd) {
                    let goc = cx >= x.w - 30 && cy >= x.h - 30;
                    x.keo = Some((p.x, p.y, x.x, x.y, x.w, goc));
                }
            });
            SetCapture(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let p = diem();
            CAC_O.with(|o| {
                if let Some(x) = o.borrow_mut().values_mut().find(|x| x.hwnd == hwnd) {
                    if let Some((mx, my, x0, y0, w0, goc)) = x.keo {
                        if goc {
                            x.w = (w0 + p.x - mx).max(150);
                        } else {
                            x.x = x0 + p.x - mx;
                            x.y = y0 + p.y - my;
                        }
                        ve_lai(x, true);
                    }
                }
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            CAC_O.with(|o| {
                if let Some(x) = o.borrow_mut().values_mut().find(|x| x.hwnd == hwnd) {
                    if x.keo.take().is_some() {
                        bao_doi(x);
                    }
                }
            });
            LRESULT(0)
        }
        WM_MOUSEWHEEL if dang_chinh() => {
            let len = ((wp.0 >> 16) & 0xFFFF) as i16 > 0;
            CAC_O.with(|o| {
                if let Some(x) = o.borrow_mut().values_mut().find(|x| x.hwnd == hwnd) {
                    x.co = (x.co + if len { 1 } else { -1 }).clamp(10, 48);
                    ve_lai(x, true);
                    bao_doi(x);
                }
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
