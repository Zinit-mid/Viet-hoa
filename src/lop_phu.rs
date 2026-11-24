//! Lớp phủ: mỗi khung chữ dịch là một cửa sổ nhỏ (click xuyên, không bị chụp),
//! chỉ vẽ lại khi nội dung đổi. Phím tắt chạy ở luồng riêng. Khung chọn vùng.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows::core::{w, HSTRING};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

#[derive(Clone, Debug, PartialEq)]
pub struct MucPhu {
    /// [x, y, w, h] toạ độ màn hình
    pub hop: [i32; 4],
    pub chu: String,
    pub co_chu: i32,
    /// (màu nền, màu chữ) khi che chữ gốc; None = hộp tối chữ trắng
    pub mau: Option<([u8; 3], [u8; 3])>,
}

/// Phím tắt (giống app cũ).
#[derive(Clone, Copy, Debug)]
pub enum PhimTat {
    /// F6 / F7: chọn lại vùng 1 (Hội thoại) / vùng 2 (Nhiệm vụ)
    ChonVung(usize),
    /// F8: bật / tắt dịch tự động
    BatTatDich,
    /// F9: dịch ngay
    DichNgay,
    /// F10: ẩn / hiện bản dịch
    AnHien,
    /// F11: chỉnh khung bằng chuột
    ChinhKhung,
    /// F12: dịch cả màn hình
    ToanMan,
    /// Ctrl+Shift+V: bật / tắt giọng đọc
    Giong,
    /// Ctrl+Shift+C: bật / tắt Claude
    Claude,
    /// Ctrl(+Shift)+mũi tên: dời khung vùng 1 (2) – (vùng, dx, dy)
    DoiKhung(usize, i32, i32),
}

const WM_VE_LAI: u32 = WM_APP + 1;
const WM_CHE_DO: u32 = WM_APP + 2;
const MAU_KHOA: COLORREF = COLORREF(0x0001_0101);
const LE: i32 = 6;

static NOI_DUNG: Mutex<Vec<MucPhu>> = Mutex::new(Vec::new());
static HWND_DIEU_KHIEN: AtomicIsize = AtomicIsize::new(0);
static GUI_PHIM: OnceLock<Mutex<Sender<PhimTat>>> = OnceLock::new();
pub static AN_CHUP_OK: AtomicBool = AtomicBool::new(true);
/// Bảng chính của app đã được ẩn khỏi ảnh chụp (đọc màn hình không thấy chữ của app).
pub static APP_DA_AN: AtomicBool = AtomicBool::new(false);

/// Dữ liệu vẽ của từng cửa sổ khung chữ (chỉ dùng trong luồng lớp phủ).
struct VeO {
    chu: Vec<u16>,
    co: i32,
    nen: [u8; 3],
    mau_chu: [u8; 3],
}

thread_local! {
    static LOP_NEN: RefCell<Option<HWND>> = const { RefCell::new(None) };
    static CAC_O: RefCell<Vec<(HWND, Option<MucPhu>)>> = const { RefCell::new(Vec::new()) };
    static VE: RefCell<HashMap<isize, VeO>> = RefCell::new(HashMap::new());
    static FONT: RefCell<HashMap<i32, HFONT>> = RefCell::new(HashMap::new());
}

fn rgb(c: [u8; 3]) -> COLORREF {
    COLORREF(c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16)
}

/// Thay toàn bộ nội dung lớp phủ (gọi từ luồng nào cũng được).
pub fn dat(muc: Vec<MucPhu>) {
    {
        let mut nd = NOI_DUNG.lock().unwrap_or_else(|e| e.into_inner());
        if *nd == muc {
            return;
        }
        *nd = muc;
    }
    let h = HWND_DIEU_KHIEN.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(h as *mut _)), WM_VE_LAI, WPARAM(0), LPARAM(0));
        }
    }
}

/// Bật/tắt chế độ đang dịch: giữ một lớp nền trong suốt toàn màn hình (để Windows
/// không phải đổi cách hiển thị game mỗi khi khung chữ hiện / ẩn → hết chớp) và
/// ẩn bảng app khỏi ảnh chụp (để F12 không đọc chữ của chính app).
pub fn bat_che_do_dich(bat: bool) {
    let h = HWND_DIEU_KHIEN.load(Ordering::Relaxed);
    if h != 0 {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(h as *mut _)),
                WM_CHE_DO,
                WPARAM(bat as usize),
                LPARAM(0),
            );
        }
    }
}

/// Nạp font Be Vietnam Pro riêng cho app.
pub fn nap_font() {
    let thu_muc = crate::duong_dan::goc().join("assets").join("fonts");
    for ten in ["BeVietnamPro-Bold.ttf", "BeVietnamPro-Regular.ttf"] {
        let p = thu_muc.join(ten);
        if p.exists() {
            unsafe {
                AddFontResourceExW(&HSTRING::from(p.as_os_str()), FR_PRIVATE, None);
            }
        }
    }
}

/// Mở luồng lớp phủ và luồng phím tắt (riêng, để Windows không gỡ bộ bắt phím).
pub fn khoi_dong(gui: Sender<PhimTat>) {
    let _ = GUI_PHIM.set(Mutex::new(gui));
    std::thread::spawn(|| unsafe { luong_phu() });
    std::thread::spawn(|| unsafe { luong_phim() });
}

fn hinst() -> HINSTANCE {
    unsafe { GetModuleHandleW(None).unwrap_or_default().into() }
}

// ---------------- luồng phím tắt ----------------

unsafe fn luong_phim() {
    match SetWindowsHookExW(WH_KEYBOARD_LL, Some(moc_phim), Some(hinst()), 0) {
        Ok(_) => crate::duong_dan::nhat_ky("Phím tắt: đã gắn bộ bắt phím"),
        Err(e) => crate::duong_dan::nhat_ky(&format!("Phím tắt: KHÔNG gắn được bộ bắt phím: {e}")),
    }
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }
}

unsafe extern "system" fn moc_phim(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && (wp.0 as u32 == WM_KEYDOWN || wp.0 as u32 == WM_SYSKEYDOWN) {
        let k = &*(lp.0 as *const KBDLLHOOKSTRUCT);
        let giu = |vk: VIRTUAL_KEY| (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0;
        let (ctrl, shift) = (giu(VK_CONTROL), giu(VK_SHIFT));
        let vung = if shift { 1 } else { 0 };
        let phim = match VIRTUAL_KEY(k.vkCode as u16) {
            VK_F6 => Some(PhimTat::ChonVung(0)),
            VK_F7 => Some(PhimTat::ChonVung(1)),
            VK_F8 => Some(PhimTat::BatTatDich),
            VK_F9 => Some(PhimTat::DichNgay),
            VK_F10 => Some(PhimTat::AnHien),
            VK_F11 => Some(PhimTat::ChinhKhung),
            VK_F12 => Some(PhimTat::ToanMan),
            VK_V if ctrl && shift => Some(PhimTat::Giong),
            VK_C if ctrl && shift => Some(PhimTat::Claude),
            VK_UP if ctrl => Some(PhimTat::DoiKhung(vung, 0, -20)),
            VK_DOWN if ctrl => Some(PhimTat::DoiKhung(vung, 0, 20)),
            VK_LEFT if ctrl => Some(PhimTat::DoiKhung(vung, -20, 0)),
            VK_RIGHT if ctrl => Some(PhimTat::DoiKhung(vung, 20, 0)),
            _ => None,
        };
        let lap_duoc = matches!(phim, Some(PhimTat::DoiKhung(..)));
        if let (Some(p), Some(g)) = (phim, GUI_PHIM.get()) {
            static CUOI: Mutex<(u32, u32)> = Mutex::new((0, 0));
            let mut c = CUOI.lock().unwrap_or_else(|e| e.into_inner());
            // chống lặp phím khi giữ: cùng phím trong 350 ms thì bỏ
            if lap_duoc || !(c.0 == k.vkCode && k.time.wrapping_sub(c.1) < 350) {
                *c = (k.vkCode, k.time);
                crate::duong_dan::nhat_ky(&format!("Phím tắt: {p:?}"));
                if let Ok(g) = g.lock() {
                    let _ = g.send(p);
                }
            } else {
                c.1 = k.time;
            }
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

// ---------------- luồng lớp phủ ----------------

unsafe fn luong_phu() {
    let h = hinst();
    let lop = WNDCLASSW {
        lpfnWndProc: Some(thu_o),
        hInstance: h,
        lpszClassName: w!("VietHoaKhungChu"),
        ..Default::default()
    };
    RegisterClassW(&lop);
    let lop_dk = WNDCLASSW {
        lpfnWndProc: Some(thu_dieu_khien),
        hInstance: h,
        lpszClassName: w!("VietHoaDieuKhien"),
        ..Default::default()
    };
    RegisterClassW(&lop_dk);
    let lop_nen = WNDCLASSW {
        lpfnWndProc: Some(thu_nen),
        hInstance: h,
        lpszClassName: w!("VietHoaLopNen"),
        hbrBackground: CreateSolidBrush(MAU_KHOA),
        ..Default::default()
    };
    RegisterClassW(&lop_nen);
    let Ok(dk) = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("VietHoaDieuKhien"),
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
    HWND_DIEU_KHIEN.store(dk.0 as isize, Ordering::Relaxed);
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }
}

unsafe extern "system" fn thu_dieu_khien(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_VE_LAI {
        cap_nhat_cac_o();
        return LRESULT(0);
    }
    if msg == WM_CHE_DO {
        let bat = wp.0 != 0;
        lop_nen(bat);
        an_app_khoi_chup(bat);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

unsafe fn font(co: i32) -> HFONT {
    FONT.with(|f| {
        *f.borrow_mut().entry(co).or_insert_with(|| {
            CreateFontW(
                -co,
                0,
                0,
                0,
                FW_BOLD.0 as i32,
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

/// Đo chữ trên 1 dòng: trả bề rộng.
unsafe fn rong_mot_dong(chu: &mut [u16], co: i32) -> i32 {
    let dc = CreateCompatibleDC(None);
    let cu = SelectObject(dc, font(co).into());
    let mut r = RECT::default();
    DrawTextW(dc, chu, &mut r, DT_SINGLELINE | DT_CALCRECT | DT_NOPREFIX);
    SelectObject(dc, cu);
    let _ = DeleteDC(dc);
    r.right
}

/// Đo chữ: trả chiều cao cần với bề rộng cho trước.
unsafe fn do_chu(chu: &mut [u16], co: i32, rong: i32) -> (i32, i32) {
    let dc = CreateCompatibleDC(None);
    let cu = SelectObject(dc, font(co).into());
    let mut r = RECT {
        left: 0,
        top: 0,
        right: rong.max(40),
        bottom: 0,
    };
    DrawTextW(dc, chu, &mut r, DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX);
    SelectObject(dc, cu);
    let _ = DeleteDC(dc);
    (r.right, r.bottom)
}

unsafe fn tao_o() -> Option<HWND> {
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        w!("VietHoaKhungChu"),
        w!("Việt Hóa"),
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
    if SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE).is_err() {
        AN_CHUP_OK.store(false, Ordering::Relaxed);
    }
    Some(hwnd)
}

unsafe fn cap_nhat_cac_o() {
    let moi: Vec<MucPhu> = NOI_DUNG
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .filter(|m| !m.chu.trim().is_empty())
        .cloned()
        .collect();
    CAC_O.with(|o| {
        let mut o = o.borrow_mut();
        while o.len() < moi.len() {
            match tao_o() {
                Some(h) => o.push((h, None)),
                None => return,
            }
        }
        // 1. Khung nào đang hiện đúng nội dung thì giữ nguyên, không động tới
        let mut dung = vec![false; o.len()];
        let mut con_lai = Vec::new();
        for m in &moi {
            match (0..o.len()).find(|&i| !dung[i] && o[i].1.as_ref() == Some(m)) {
                Some(i) => dung[i] = true,
                None => con_lai.push(m),
            }
        }
        // 2. Nội dung mới: ưu tiên khung đang ẩn, rồi khung cũ ở gần chỗ đó nhất
        for m in con_lai {
            let i = (0..o.len())
                .filter(|&i| !dung[i])
                .min_by_key(|&i| match &o[i].1 {
                    None => 0,
                    Some(c) => 1 + (c.hop[0] - m.hop[0]).abs() + (c.hop[1] - m.hop[1]).abs(),
                });
            let Some(i) = i else { break };
            dung[i] = true;
            ve_o(o[i].0, m);
            o[i].1 = Some(m.clone());
        }
        // 3. Khung không còn dùng → ẩn
        for (i, (hwnd, cu)) in o.iter_mut().enumerate() {
            if !dung[i] && cu.is_some() {
                *cu = None;
                let _ = ShowWindow(*hwnd, SW_HIDE);
            }
        }
    });
}

unsafe fn ve_o(hwnd: HWND, m: &MucPhu) {
    let (nen, mau_chu, alpha) = match m.mau {
        Some((n, c)) => (n, c, 255u8),
        None => ([16, 16, 20], [255, 255, 255], 225u8),
    };
    let mut chu: Vec<u16> = m.chu.encode_utf16().collect();
    let co = m.co_chu.clamp(10, 60);
    // Giữ nguyên cỡ chữ. Chữ Việt dài hơn tiếng Anh thì kéo khung rộng ra (tối đa
    // ~2,5 lần, không vượt mép màn hình) để nằm trên 1 dòng; vẫn dài thì xuống dòng.
    let man_w = GetSystemMetrics(SM_CXSCREEN);
    let goc_w = m.hop[2].max(40);
    let toi_da = (goc_w * 5 / 2).max(320).min(man_w - m.hop[0]).max(goc_w);
    let can = rong_mot_dong(&mut chu, co) + 2 * LE + 2;
    let rong = if can <= toi_da {
        can.max(goc_w)
    } else {
        toi_da
    };
    let (_, cao_chu) = do_chu(&mut chu, co, rong - 2 * LE);
    let cao = m.hop[3].max(cao_chu + 2 * LE);
    VE.with(|v| {
        v.borrow_mut().insert(
            hwnd.0 as isize,
            VeO {
                chu,
                co,
                nen,
                mau_chu,
            },
        );
    });
    let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA);
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        m.hop[0],
        m.hop[1],
        rong,
        cao,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    let _ = InvalidateRect(Some(hwnd), None, false);
}

/// Lớp nền trong suốt toàn màn hình (màu khóa = trong suốt hoàn toàn, click xuyên).
unsafe fn lop_nen(bat: bool) {
    LOP_NEN.with(|l| {
        let mut l = l.borrow_mut();
        if bat && l.is_none() {
            let (w, h) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            if let Ok(hwnd) = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                w!("VietHoaLopNen"),
                w!("Việt Hóa – lớp nền"),
                WS_POPUP,
                0,
                0,
                w,
                h,
                None,
                None,
                Some(hinst()),
                None,
            ) {
                let _ = SetLayeredWindowAttributes(hwnd, MAU_KHOA, 255, LWA_COLORKEY);
                let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
                *l = Some(hwnd);
            }
        }
        if let Some(hwnd) = *l {
            let _ = ShowWindow(hwnd, if bat { SW_SHOWNOACTIVATE } else { SW_HIDE });
        }
    });
}

unsafe extern "system" fn thu_nen(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_NCHITTEST {
        return LRESULT(HTTRANSPARENT as isize);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// Ẩn / hiện các cửa sổ giao diện của app (không phải khung chữ) khỏi ảnh chụp.
unsafe fn an_app_khoi_chup(an: bool) {
    unsafe extern "system" fn moi_cua_so(h: HWND, lp: LPARAM) -> windows::core::BOOL {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        // Chỉ bảng chính đang hiện của app. Không đụng cửa sổ ẩn (driver OpenGL
        // tạo "NVOGLDC invisible", "__wglDummyWindowFodder" – đụng vào là hiện bảng trắng)
        if pid == windows::Win32::System::Threading::GetCurrentProcessId()
            && IsWindowVisible(h).as_bool()
        {
            let mut ten = [0u16; 64];
            let n = GetClassNameW(h, &mut ten) as usize;
            let lop = String::from_utf16_lossy(&ten[..n]);
            let mut tieu_de = [0u16; 64];
            let m = GetWindowTextW(h, &mut tieu_de) as usize;
            if !lop.starts_with("VietHoa") && String::from_utf16_lossy(&tieu_de[..m]) == "Việt Hóa"
            {
                let ds = &mut *(lp.0 as *mut Vec<HWND>);
                ds.push(h);
            }
        }
        windows::core::BOOL(1)
    }
    let mut ds: Vec<HWND> = Vec::new();
    let _ = EnumWindows(Some(moi_cua_so), LPARAM(&mut ds as *mut _ as isize));
    let mut da_an = false;
    for h in ds {
        da_an |= SetWindowDisplayAffinity(h, if an { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE })
            .is_ok()
            && an;
    }
    APP_DA_AN.store(da_an, Ordering::Relaxed);
}

unsafe extern "system" fn thu_o(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            VE.with(|v| {
                let mut v = v.borrow_mut();
                let Some(ve) = v.get_mut(&(hwnd.0 as isize)) else {
                    return;
                };
                let co_nen = CreateSolidBrush(rgb(ve.nen));
                FillRect(hdc, &rc, co_nen);
                let _ = DeleteObject(co_nen.into());
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, rgb(ve.mau_chu));
                let cu = SelectObject(hdc, font(ve.co).into());
                let mut trong = RECT {
                    left: LE,
                    top: LE,
                    right: rc.right - LE,
                    bottom: rc.bottom - LE,
                };
                let mut do_rc = trong;
                DrawTextW(
                    hdc,
                    &mut ve.chu,
                    &mut do_rc,
                    DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX,
                );
                let du = (trong.bottom - trong.top) - (do_rc.bottom - do_rc.top);
                if du > 0 {
                    trong.top += du / 2;
                }
                DrawTextW(hdc, &mut ve.chu, &mut trong, DT_WORDBREAK | DT_NOPREFIX);
                SelectObject(hdc, cu);
            });
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ---------------- Chọn vùng bằng chuột ----------------

static CHON: Mutex<(Option<POINT>, Option<POINT>, bool)> = Mutex::new((None, None, false));
static TEN_CHON: Mutex<String> = Mutex::new(String::new());

/// Hiện màn mờ, người dùng kéo chuột chọn vùng. Esc / chuột phải = hủy.
/// Chặn luồng gọi cho tới khi chọn xong.
pub fn chon_vung(ten: &str) -> Option<[i32; 4]> {
    *CHON.lock().unwrap() = (None, None, false);
    *TEN_CHON.lock().unwrap() = ten.to_uppercase();
    unsafe {
        let h = hinst();
        let lop = WNDCLASSW {
            lpfnWndProc: Some(thu_chon),
            hInstance: h,
            lpszClassName: w!("VietHoaChonVung"),
            hCursor: LoadCursorW(None, IDC_CROSS).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&lop);
        let (w, hh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("VietHoaChonVung"),
            w!("Chọn vùng"),
            WS_POPUP,
            0,
            0,
            w,
            hh,
            None,
            None,
            Some(h),
            None,
        )
        .ok()?;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), 110, LWA_ALPHA);
        let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = DestroyWindow(hwnd);
    }
    let (a, b, xong) = *CHON.lock().unwrap();
    let (a, b) = (a?, b?);
    if !xong {
        return None;
    }
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
    (w >= 20 && h >= 10).then_some([x, y, w, h])
}

unsafe extern "system" fn thu_chon(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let diem = || POINT {
        x: (lp.0 & 0xFFFF) as i16 as i32,
        y: ((lp.0 >> 16) & 0xFFFF) as i16 as i32,
    };
    match msg {
        WM_LBUTTONDOWN => {
            let p = diem();
            *CHON.lock().unwrap() = (Some(p), Some(p), false);
            SetCapture(hwnd);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let mut c = CHON.lock().unwrap();
            if c.0.is_some() && !c.2 {
                c.1 = Some(diem());
                drop(c);
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let mut c = CHON.lock().unwrap();
            c.1 = Some(diem());
            c.2 = true;
            drop(c);
            let _ = ReleaseCapture();
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_RBUTTONDOWN => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_KEYDOWN if wp.0 as u16 == VK_ESCAPE.0 => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let den = CreateSolidBrush(COLORREF(0));
            FillRect(hdc, &rc, den);
            let _ = DeleteObject(den.into());
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, COLORREF(0x00FF_FFFF));
            let cu = SelectObject(hdc, font(26).into());
            let mut huong_dan: Vec<u16> = format!(
                "Kéo chuột khoanh vùng chữ \"{}\"  •  Esc / chuột phải: hủy",
                TEN_CHON.lock().unwrap()
            )
            .encode_utf16()
            .collect();
            let mut o = RECT {
                left: 0,
                top: 40,
                right: rc.right,
                bottom: 100,
            };
            DrawTextW(hdc, &mut huong_dan, &mut o, DT_CENTER | DT_NOPREFIX);
            SelectObject(hdc, cu);
            let c = *CHON.lock().unwrap();
            if let (Some(a), Some(b)) = (c.0, c.1) {
                let r = RECT {
                    left: a.x.min(b.x),
                    top: a.y.min(b.y),
                    right: a.x.max(b.x),
                    bottom: a.y.max(b.y),
                };
                let trang = CreateSolidBrush(COLORREF(0x00FF_FFFF));
                FrameRect(hdc, &r, trang);
                let r2 = RECT {
                    left: r.left + 1,
                    top: r.top + 1,
                    right: r.right - 1,
                    bottom: r.bottom - 1,
                };
                FrameRect(hdc, &r2, trang);
                let _ = DeleteObject(trang.into());
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
