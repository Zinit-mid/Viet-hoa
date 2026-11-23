//! Vòng dịch màn hình – làm theo app Dịch Game cũ (_vong_lap, _xu_ly_vung,
//! _thuc_hien_dich, _dich_nhanh, _dich_som) + F12 dịch cả màn hình.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::cau_hinh::{self, CauHinh, Vung, SO_VUNG_TOI_DA, VONG_CO_DINH};
use crate::chup::Anh;
use crate::dich::bo_nho::{chu_giong, so_hoa};
use crate::dich::{self, May};
use crate::khung_dich::{self, KhungDoi, KhungHien, Kieu};
use crate::lop_phu::{self, MucPhu, PhimTat};
use crate::ocr::{self, Dong, Ocr};
use crate::quan_ly_chup::QuanLyChup;
use crate::{che_mau, giong_doc};

/// Lệnh gửi vào luồng dịch (từ giao diện, phím tắt, khung dịch).
#[derive(Debug)]
pub enum Lenh {
    Phim(PhimTat),
    /// Đã chọn xong vùng chữ (chỉ số, toạ độ)
    VungMoi(usize, [i32; 4]),
    /// Người dùng kéo / đổi cỡ khung dịch
    KhungDoi(KhungDoi),
}

#[derive(Default)]
pub struct TinhTrang {
    pub may_chup: String,
    pub ocr: String,
    pub loi: String,
    pub cuoi_en: String,
    pub cuoi_vi: String,
    pub cuoi_may: String,
    pub cuoi_ms: u128,
    pub so_doan_toan_man: usize,
}

pub struct Chung {
    pub cau_hinh: Mutex<CauHinh>,
    pub tinh_trang: Mutex<TinhTrang>,
    /// F8: dịch tự động các vùng
    pub dang_dich: AtomicBool,
    /// F12: dịch cả màn hình
    pub toan_man: AtomicBool,
    /// F10: ẩn bản dịch
    pub an_ban_dich: AtomicBool,
    pub dang_chon_vung: AtomicBool,
    /// Giao diện cần lưu cấu hình
    pub can_luu: AtomicBool,
}

impl Chung {
    pub fn moi(ch: CauHinh) -> Arc<Self> {
        let tu_dong = ch.tu_dong_dich;
        dich::claude::BAT.store(ch.claude_bat, Ordering::Relaxed);
        Arc::new(Self {
            cau_hinh: Mutex::new(ch),
            tinh_trang: Mutex::new(TinhTrang::default()),
            dang_dich: AtomicBool::new(tu_dong),
            toan_man: AtomicBool::new(false),
            an_ban_dich: AtomicBool::new(false),
            dang_chon_vung: AtomicBool::new(false),
            can_luu: AtomicBool::new(false),
        })
    }
    pub fn ch(&self) -> std::sync::MutexGuard<'_, CauHinh> {
        self.cau_hinh.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn tt(&self) -> std::sync::MutexGuard<'_, TinhTrang> {
        self.tinh_trang.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Mở khung chọn vùng chữ ở luồng riêng; kết quả gửi về luồng dịch.
pub fn chon_vung(chung: &Arc<Chung>, gui: &Sender<Lenh>, chi_so: usize) {
    if chung.dang_chon_vung.swap(true, Ordering::SeqCst) {
        return;
    }
    let (chung, gui) = (chung.clone(), gui.clone());
    std::thread::spawn(move || {
        let ten = chung.ch().cac_vung.get(chi_so).map(|v| v.ten.clone()).unwrap_or_else(|| "vùng mới".into());
        if let Some(v) = lop_phu::chon_vung(&ten) {
            let _ = gui.send(Lenh::VungMoi(chi_so, v));
        }
        chung.dang_chon_vung.store(false, Ordering::SeqCst);
    });
}

fn nk(s: &str) {
    if !cfg!(test) {
        crate::duong_dan::nhat_ky(s);
    }
}

pub fn het_cau(s: &str) -> bool {
    s.trim_end().ends_with(['.', '!', '?', '…', '"', '”', '’'])
}

// ---------------- trạng thái một vùng (giống TrangThai của app cũ) ----------------

#[derive(Default)]
struct TrangThai {
    van_tay: Option<Vec<u8>>,
    chu_ocr: Option<String>,
    chu_cho: Option<String>,
    so_lan_giong: u32,
    /// Câu tiếng Anh đã gửi dịch / đang hiện
    chu_da_hien: String,
    lan_trong: u32,
    lan_lech: u32,
    t_xem_som: Option<Instant>,
    t_doi_chu: Option<Instant>,
    /// Ô chữ trong vùng (toạ độ vùng), (màu nền, màu chữ), cao 1 dòng – để che chữ gốc
    che: Option<([i32; 4], Option<([u8; 3], [u8; 3])>, i32)>,
    /// Bản dịch đang hiện
    van_ban: String,
}

type TT = Arc<Mutex<TrangThai>>;

fn khoa(t: &TT) -> std::sync::MutexGuard<'_, TrangThai> {
    t.lock().unwrap_or_else(|e| e.into_inner())
}

/// Kết quả gửi về vòng chính để hiện.
struct Hien {
    id: String,
    /// Câu tiếng Anh của bản dịch này (chỉ hiện nếu vẫn là câu đang hiện)
    en: String,
    vi: String,
    may: Option<May>,
    tam: bool,
    luc: Instant,
}

// ---------------- việc nền "chỉ làm việc MỚI NHẤT" (giống ViecNen) ----------------

struct ViecNen<T: Send + 'static> {
    cho: Arc<(Mutex<Option<T>>, Condvar)>,
}

impl<T: Send + 'static> ViecNen<T> {
    fn moi(mut ham: impl FnMut(T) + Send + 'static) -> Self {
        let cho: Arc<(Mutex<Option<T>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
        let c = cho.clone();
        std::thread::spawn(move || loop {
            let viec = {
                let mut g = c.0.lock().unwrap_or_else(|e| e.into_inner());
                while g.is_none() {
                    g = c.1.wait(g).unwrap_or_else(|e| e.into_inner());
                }
                g.take().unwrap()
            };
            ham(viec);
        });
        Self { cho }
    }
    fn gui(&self, viec: T) {
        *self.cho.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(viec);
        self.cho.1.notify_one();
    }
}

struct ViecDich {
    v: Vung,
    chu: String,
    che_do: String,
    giong: bool,
    /// Đang hiện bản offline/Google → đổi sang bản Claude khi có
    thay_claude: bool,
    hien_google_truoc: bool,
}

/// Dịch một câu của vùng (giống _thuc_hien_dich + _dich_nhanh của app cũ).
fn thuc_hien_dich(job: ViecDich, st: &TT, gui: &Sender<Hien>) {
    let ViecDich { v, chu, che_do, giong, thay_claude, hien_google_truoc } = job;
    let luc = Instant::now();
    let con_dung = || khoa(st).chu_da_hien == chu;
    let phu_de = v.loai == "phu_de";
    let gui_hien = |vi: String, may: Option<May>, tam: bool| {
        let _ = gui.send(Hien { id: v.id.clone(), en: chu.clone(), vi, may, tam, luc });
    };
    let che_do = if che_do == "claude" && !dich::claude::co_khoa() { "offline_claude".to_string() } else { che_do };

    if che_do != "claude" {
        // Hiện bản dịch NGAY: Việt hóa game > Claude đã lưu > offline > Google
        let che_do_vung = if phu_de { che_do.as_str() } else if che_do == "google" { "google" } else { "offline_claude" };
        let Some((vi, may)) = dich::dich_nhanh(&chu, che_do_vung) else {
            if con_dung() {
                khoa(st).chu_da_hien.clear(); // lần sau thử lại
            }
            return;
        };
        if con_dung() {
            gui_hien(vi.clone(), Some(may), false);
            if giong && phu_de {
                giong_doc::doc(vi);
            }
        }
        // Claude dịch ngầm để lưu; lần sau gặp lại câu này hiện bản Claude ngay
        if che_do_vung == "offline_claude" && matches!(may, May::Offline | May::Google) {
            if let Some(vi) = dich::claude_luu_ngam(&chu) {
                if thay_claude && con_dung() {
                    gui_hien(vi, Some(May::Claude), false);
                }
            }
        }
        return;
    }

    // Chế độ Claude: bản đã lưu → (Google hiện trước) → Claude
    if let Some(vi) = dich::tim_da_luu(&chu) {
        if con_dung() {
            gui_hien(vi.clone(), Some(May::DaLuu), false);
            if giong && phu_de {
                giong_doc::doc(vi);
            }
        }
        return;
    }
    if hien_google_truoc && phu_de {
        let (chu2, gui2, id2) = (chu.clone(), gui.clone(), v.id.clone());
        let st2 = st.clone();
        std::thread::spawn(move || {
            if let Some(vi) = dich::google_lo(&[chu2.clone()]).pop().flatten() {
                if khoa(&st2).chu_da_hien == chu2 {
                    let _ = gui2.send(Hien { id: id2, en: chu2, vi, may: Some(May::Google), tam: true, luc });
                }
            }
        });
    }
    match dich::dich_nhanh(&chu, "claude") {
        Some((vi, may)) if con_dung() => {
            gui_hien(vi.clone(), Some(may), false);
            if giong && phu_de {
                giong_doc::doc(vi);
            }
        }
        Some(_) => {}
        None => {
            if con_dung() {
                khoa(st).chu_da_hien.clear();
            }
        }
    }
}

// ---------------- đọc một vùng (giống _xu_ly_vung của app cũ) ----------------

struct NguCanh<'a> {
    nguong: u8,
    gui_hien: &'a Sender<Hien>,
}

/// Trả Some(câu) khi cần gửi dịch.
fn xu_ly_vung(ocr: &Ocr, v: &Vung, anh: &Anh, ep: bool, st: &TT, nc: &NguCanh) -> Option<String> {
    let tuy = cau_hinh::tuy_loai(&v.loai);
    let mut anh_doc = anh.clone();
    if v.loc_trang {
        anh_doc.loc_trang(nc.nguong);
    }
    let van_tay = anh_doc.van_tay();
    let mut s = khoa(st);
    let hinh_dung_yen = s.van_tay.as_ref().is_some_and(|cu| {
        cu.len() == van_tay.len()
            && cu.iter().zip(&van_tay).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs()).sum::<u32>() as f32
                / (van_tay.len() as f32)
                < 3.0
    });
    let chu = if !ep && s.chu_ocr.is_some() && hinh_dung_yen {
        s.chu_ocr.clone().unwrap_or_default()
    } else {
        drop(s);
        let dong: Vec<Dong> = match ocr.doc(&anh_doc) {
            Ok(d) => d,
            Err(e) => {
                nk(&format!("OCR lỗi ở vùng {}: {e}", v.ten));
                return None;
            }
        };
        let chu = ocr::lam_sach(&dong.iter().map(|d| d.chu.clone()).collect::<Vec<_>>());
        let o: Vec<&Dong> = dong.iter().filter(|d| ocr::dong_hop_le(&d.chu)).collect();
        s = khoa(st);
        s.che = if o.is_empty() {
            None
        } else {
            let x0 = o.iter().map(|d| d.hop[0]).min().unwrap();
            let y0 = o.iter().map(|d| d.hop[1]).min().unwrap();
            let x1 = o.iter().map(|d| d.hop[0] + d.hop[2]).max().unwrap();
            let y1 = o.iter().map(|d| d.hop[1] + d.hop[3]).max().unwrap();
            let hop = [x0, y0, x1 - x0, y1 - y0];
            Some((hop, che_mau::mau_che(anh, hop), (y1 - y0) / o.len() as i32))
        };
        s.chu_ocr = Some(chu.clone());
        chu
    };
    s.van_tay = Some(van_tay);
    // Chữ tiếng Việt (kể cả bị OCR đọc mất dấu) → không dịch, coi như trống
    let chu = if dich::the::la_tieng_viet(&chu) || dich::the::la_viet_mat_dau(&chu) { String::new() } else { chu };

    // ẩn khi trống / lệch lâu
    s.lan_trong = if chu.is_empty() { s.lan_trong + 1 } else { 0 };
    if !s.chu_da_hien.is_empty() && !chu.is_empty() && !chu_giong(&so_hoa(&chu), &so_hoa(&s.chu_da_hien), tuy.lech) {
        s.lan_lech += 1;
    } else {
        s.lan_lech = 0;
    }
    let dang_hien = !s.chu_da_hien.is_empty() || !s.van_ban.is_empty();
    if dang_hien && (s.lan_trong >= tuy.an_trong || (!s.chu_da_hien.is_empty() && s.lan_lech >= tuy.an_lech)) {
        s.chu_da_hien.clear();
        s.van_ban.clear();
        s.lan_lech = 0;
    }

    if s.chu_cho.as_ref().is_some_and(|c| chu_giong(&so_hoa(&chu), &so_hoa(c), 0.9)) {
        s.so_lan_giong += 1;
    } else {
        s.so_lan_giong = 0;
        if !chu.is_empty() {
            s.t_doi_chu = Some(Instant::now());
        }
    }
    s.chu_cho = Some(chu.clone());
    let phu_de = v.loai == "phu_de";
    let on_dinh = ep || s.so_lan_giong >= 1 || (phu_de && het_cau(&chu)) || (!phu_de && hinh_dung_yen);

    // chữ đang chạy từng chữ: hiện bản tạm (offline / Google) trước
    if !on_dinh && phu_de && chu.chars().count() >= 30 && s.t_xem_som.is_none_or(|t| t.elapsed().as_millis() > 600) {
        s.t_xem_som = Some(Instant::now());
        let (chu2, st2, gui2, id) = (chu.clone(), st.clone(), nc.gui_hien.clone(), v.id.clone());
        std::thread::spawn(move || {
            let vi = if dich::offline::co_the_dung() {
                dich::dich_nhieu(&[chu2.clone()], false).pop().flatten()
            } else {
                dich::google_lo(&[chu2.clone()]).pop().flatten()
            };
            let Some(vi) = vi else { return };
            let s = khoa(&st2);
            let con_moi = s.chu_cho.as_ref().is_some_and(|c| {
                let dau: String = c.chars().take(chu2.chars().count()).collect();
                chu_giong(&chu2, &dau, 0.85)
            });
            let da_co_ban_chinh = s.chu_da_hien.starts_with(&chu2.chars().take(20).collect::<String>())
                && s.chu_da_hien.len() >= chu2.len();
            if con_moi && !da_co_ban_chinh {
                drop(s);
                let _ = gui2.send(Hien { id, en: String::new(), vi, may: None, tam: true, luc: Instant::now() });
            }
        });
    }
    if !on_dinh || chu.is_empty() {
        return None;
    }
    if !ep && !s.chu_da_hien.is_empty() {
        if chu == s.chu_da_hien {
            return None;
        }
        let chi_doi_so = so_hoa(&chu) == so_hoa(&s.chu_da_hien);
        if !chi_doi_so && chu_giong(&so_hoa(&chu), &so_hoa(&s.chu_da_hien), tuy.bo_qua) {
            return None;
        }
    }
    s.chu_da_hien = chu.clone();
    Some(chu)
}

// ---------------- F12: dịch cả màn hình ----------------

struct Doan {
    en: String,
    hop: [i32; 4],
    cao: i32,
    lan_thay: u32,
    mat: u32,
    vi: Option<String>,
    da_claude: bool,
    mau: Option<([u8; 3], [u8; 3])>,
}

fn chong(a: [i32; 4], b: [i32; 4]) -> f32 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    if w <= 0 || h <= 0 {
        return 0.0;
    }
    (w * h) as f32 / (a[2] * a[3]).min(b[2] * b[3]).max(1) as f32
}

fn trong_vung(h: [i32; 4], v: [i32; 4]) -> bool {
    let (cx, cy) = (h[0] + h[2] / 2, h[1] + h[3] / 2);
    cx >= v[0] && cx <= v[0] + v[2] && cy >= v[1] && cy <= v[1] + v[3]
}

fn cap_nhat_doan(doan: &mut Vec<Doan>, moi: Vec<Dong>, anh: &Anh, gui: &Sender<(Vec<String>, Vec<Option<String>>)>, che_do: &str) {
    let mut da_khop = vec![false; doan.len()];
    let mut them = Vec::new();
    let mut can_nhanh: Vec<String> = Vec::new();
    for d in moi {
        let hop_anh = [d.hop[0] - anh.x0, d.hop[1] - anh.y0, d.hop[2], d.hop[3]];
        let vi_tri = doan
            .iter()
            .enumerate()
            .filter(|(i, _)| !da_khop[*i])
            .map(|(i, p)| (i, chong(p.hop, d.hop)))
            .filter(|(_, c)| *c >= 0.5)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        match vi_tri {
            Some(i) => {
                da_khop[i] = true;
                let p = &mut doan[i];
                p.lan_thay += 1;
                p.mat = 0;
                if !chu_giong(&p.en, &d.chu, 0.75) {
                    p.en = d.chu;
                    p.hop = d.hop;
                    p.cao = d.cao_dong;
                    p.mau = che_mau::mau_che(anh, hop_anh);
                    p.lan_thay = 1;
                    p.da_claude = false;
                    p.vi = dich::tim_da_luu(&p.en);
                    if p.vi.is_none() {
                        can_nhanh.push(p.en.clone());
                    }
                } else if (p.hop[0] - d.hop[0]).abs() > 8 || (p.hop[1] - d.hop[1]).abs() > 8 {
                    p.hop = d.hop;
                }
            }
            None => {
                let vi = dich::tim_da_luu(&d.chu);
                if vi.is_none() {
                    can_nhanh.push(d.chu.clone());
                }
                them.push(Doan {
                    mau: che_mau::mau_che(anh, hop_anh),
                    vi,
                    cao: d.cao_dong,
                    en: d.chu,
                    hop: d.hop,
                    lan_thay: 1,
                    mat: 0,
                    da_claude: false,
                });
            }
        }
    }
    for (i, p) in doan.iter_mut().enumerate() {
        if !da_khop[i] {
            p.mat += 1;
        }
    }
    doan.retain(|p| p.mat < 3);
    doan.extend(them);
    if !can_nhanh.is_empty() {
        let gui = gui.clone();
        let chi_offline = che_do.starts_with("offline");
        std::thread::spawn(move || {
            let vi = if chi_offline { dich::dich_nhieu(&can_nhanh, false) } else { dich::google_lo(&can_nhanh) };
            let _ = gui.send((can_nhanh, vi));
        });
    }
    // Đoạn tồn tại ≥ 2 lần quét: Claude dịch lô (lưu bộ nhớ)
    if che_do != "google" && che_do != "offline" && dich::claude::co_khoa() {
        let lo: Vec<String> = doan
            .iter_mut()
            .filter(|p| p.lan_thay >= 2 && !p.da_claude)
            .map(|p| {
                p.da_claude = true;
                p.en.clone()
            })
            .collect();
        if !lo.is_empty() {
            let gui = gui.clone();
            std::thread::spawn(move || {
                let vi = dich::dich_nhieu(&lo, true);
                let _ = gui.send((lo, vi));
            });
        }
    }
}

// ---------------- vòng chính ----------------

pub fn chay(chung: Arc<Chung>, nhan: Receiver<Lenh>, gui: Sender<Lenh>) {
    let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED) };
    let mut ql = QuanLyChup::default();
    nk(&format!("Khởi động luồng dịch; bộ nhớ dịch {} mục", dich::nap_san()));
    let ocr = match Ocr::moi() {
        Ok(o) => {
            chung.tt().ocr = "Sẵn sàng".into();
            Some(o)
        }
        Err(e) => {
            nk(&format!("OCR lỗi: {e}"));
            chung.tt().ocr = e.to_string();
            None
        }
    };
    if chung.dang_dich.load(Ordering::SeqCst) && chung.ch().che_do_hoi_thoai.starts_with("offline") {
        dich::offline::nap_san();
    }
    let (gui_hien, nhan_hien) = channel::<Hien>();
    let (gui_tm, nhan_tm) = channel::<(Vec<String>, Vec<Option<String>>)>();
    let mut trang_thai: HashMap<String, TT> = HashMap::new();
    let mut viec_dich: HashMap<String, ViecNen<ViecDich>> = HashMap::new();
    let mut viec_doc: HashMap<String, ViecNen<(Vung, Anh, bool)>> = HashMap::new();
    let mut doan: Vec<Doan> = Vec::new();
    let mut dem: u64 = 0;
    let mut quet_tm_cuoi = Instant::now() - Duration::from_secs(5);
    let mut ep = true;
    let mut hoat_dong = false;
    let mut ap_lai = Instant::now();
    let mut bat_tu = Instant::now();

    loop {
        let bat_dau = Instant::now();
        // 1. Lệnh
        while let Ok(l) = nhan.try_recv() {
            if !matches!(l, Lenh::Phim(PhimTat::DoiKhung(..))) {
                nk(&format!("Lệnh: {l:?}"));
            }
            match l {
                Lenh::Phim(PhimTat::ChonVung(i)) => chon_vung(&chung, &gui, i),
                Lenh::Phim(PhimTat::BatTatDich) => {
                    let b = !chung.dang_dich.load(Ordering::SeqCst);
                    chung.dang_dich.store(b, Ordering::SeqCst);
                    chung.ch().tu_dong_dich = b;
                    chung.can_luu.store(true, Ordering::SeqCst);
                    if b {
                        ep = true;
                        dich::offline::nap_san();
                    }
                }
                Lenh::Phim(PhimTat::DichNgay) => ep = true,
                Lenh::Phim(PhimTat::AnHien) => {
                    let b = !chung.an_ban_dich.load(Ordering::SeqCst);
                    chung.an_ban_dich.store(b, Ordering::SeqCst);
                }
                Lenh::Phim(PhimTat::ChinhKhung) => khung_dich::chinh(!khung_dich::dang_chinh()),
                Lenh::Phim(PhimTat::ToanMan) => {
                    let b = !chung.toan_man.load(Ordering::SeqCst);
                    chung.toan_man.store(b, Ordering::SeqCst);
                    doan.clear();
                    if b {
                        dich::offline::nap_san();
                    }
                }
                Lenh::Phim(PhimTat::Giong) => {
                    let mut ch = chung.ch();
                    ch.giong_doc = !ch.giong_doc;
                    if !ch.giong_doc {
                        giong_doc::dung();
                    }
                    chung.can_luu.store(true, Ordering::SeqCst);
                }
                Lenh::Phim(PhimTat::Claude) => {
                    let mut ch = chung.ch();
                    ch.claude_bat = !ch.claude_bat;
                    dich::claude::BAT.store(ch.claude_bat, Ordering::Relaxed);
                    chung.can_luu.store(true, Ordering::SeqCst);
                }
                Lenh::Phim(PhimTat::DoiKhung(i, dx, dy)) => {
                    let mut ch = chung.ch();
                    if let Some(v) = ch.cac_vung.get_mut(i) {
                        if let Some(mut k) = v.khung_dich() {
                            k[0] += dx;
                            k[1] += dy;
                            v.khung = Some(k);
                            chung.can_luu.store(true, Ordering::SeqCst);
                        }
                    }
                }
                Lenh::KhungDoi(k) => {
                    let mut ch = chung.ch();
                    if let Some(v) = ch.cac_vung.iter_mut().find(|v| v.id == k.id) {
                        v.khung = Some(k.khung);
                        v.co_chu = k.co_chu;
                        chung.can_luu.store(true, Ordering::SeqCst);
                    }
                }
                Lenh::VungMoi(i, r) => {
                    let mut ch = chung.ch();
                    if i >= ch.cac_vung.len() && ch.cac_vung.len() < SO_VUNG_TOI_DA {
                        let so = (1..).find(|n| !ch.cac_vung.iter().any(|v| v.id == format!("vung_{n}"))).unwrap_or(1);
                        let ten = format!("Vùng {}", ch.cac_vung.len() + 1);
                        ch.cac_vung.push(cau_hinh::vung_moi(&format!("vung_{so}"), &ten, "co_dinh"));
                    }
                    let n = ch.cac_vung.len();
                    if let Some(v) = ch.cac_vung.get_mut(i.min(n.saturating_sub(1))) {
                        v.vung = Some(r);
                        v.khung = Some([r[0], r[1], r[2]]); // khung dịch đổi theo vùng mới
                        trang_thai.remove(&v.id);
                        nk(&format!("Đã chọn vùng {}: {r:?}", v.ten));
                    }
                    chung.can_luu.store(true, Ordering::SeqCst);
                    chung.dang_dich.store(true, Ordering::SeqCst);
                    ep = true;
                }
            }
        }

        let (cfg_vung, che_goc, kieu_hien, che_do, giong, (thay_claude, thay_co_dinh), google_truoc, vien, do_dam_chu, do_mo, nguong, co_tm, chu_ky) = {
            let ch = chung.ch();
            (
                {
                    giong_doc::dat_giong(&ch.giong);
                    ch.cac_vung.clone()
                },
                ch.che_chu_goc,
                ch.kieu_hien.clone(),
                ch.che_do_hoi_thoai.clone(),
                ch.giong_doc,
                (ch.thay_bang_claude, ch.co_dinh_thay_claude),
                ch.hien_google_truoc,
                ch.vien_chu,
                ch.do_dam_chu,
                ch.do_mo,
                ch.nguong_chu_trang,
                ch.co_chu_toan_man,
                ch.chu_ky_giay,
            )
        };

        // 2. Bản dịch mới
        while let Ok(h) = nhan_hien.try_recv() {
            let Some(st) = trang_thai.get(&h.id) else { continue };
            let mut s = khoa(st);
            if h.en.is_empty() || s.chu_da_hien == h.en {
                s.van_ban = h.vi.clone();
            }
            drop(s);
            if !h.tam {
                if let Some(may) = h.may {
                    nk(&format!("Dịch xong [{}] ({}, {} ms): {} => {}", h.id, may.ten(), h.luc.elapsed().as_millis(), h.en, h.vi));
                    let mut tt = chung.tt();
                    tt.cuoi_ms = h.luc.elapsed().as_millis();
                    tt.cuoi_en = h.en;
                    tt.cuoi_vi = h.vi;
                    tt.cuoi_may = may.ten().into();
                }
            }
        }
        while let Ok((en, vi)) = nhan_tm.try_recv() {
            nk(&format!("F12: dịch xong {}/{} đoạn", vi.iter().filter(|v| v.is_some()).count(), en.len()));
            for (e, v) in en.iter().zip(vi) {
                if let (Some(d), Some(v)) = (doan.iter_mut().find(|d| &d.en == e), v) {
                    d.vi = Some(v);
                }
            }
        }

        let dang_dich = chung.dang_dich.load(Ordering::SeqCst);
        let toan_man = chung.toan_man.load(Ordering::SeqCst);
        let chinh = khung_dich::dang_chinh();
        if (dang_dich || toan_man) != hoat_dong {
            hoat_dong = dang_dich || toan_man;
            bat_tu = Instant::now();
            lop_phu::bat_che_do_dich(hoat_dong);
            nk(if hoat_dong { "Bắt đầu dịch" } else { "Dừng dịch" });
        } else if hoat_dong && ap_lai.elapsed() >= Duration::from_millis(if lop_phu::APP_DA_AN.load(Ordering::Relaxed) { 2000 } else { 200 }) {
            // áp lại "ẩn bảng app khỏi ảnh chụp" (cửa sổ app có thể tạo sau)
            ap_lai = Instant::now();
            lop_phu::bat_che_do_dich(true);
        }

        // 3. Đọc màn hình (khi đang chọn vùng / chỉnh khung thì tạm dừng đọc)
        // vừa bật: chờ bảng app kịp ẩn khỏi ảnh chụp rồi mới đọc (tránh đọc chữ của app)
        let cho_an = !lop_phu::APP_DA_AN.load(Ordering::Relaxed) && bat_tu.elapsed() < Duration::from_secs(3);
        let tam_dung = chung.dang_chon_vung.load(Ordering::SeqCst) || chinh || cho_an;
        let mut co_moi = false;
        if let (Some(ocr), true, false) = (ocr.as_ref(), dang_dich || toan_man, tam_dung) {
            let kq = match ql.lay_may(&chung) {
                Some(m) => Some(m.cap_nhat()),
                None => None,
            };
            match kq {
                Some(Ok(b)) => {
                    ql.thanh_cong(&chung);
                    co_moi = b;
                }
                Some(Err(e)) => ql.loi(&chung, &e.to_string()),
                None => {}
            }
            if let Some(may) = ql.may.as_mut() {
                dem += 1;
                let ep_lan_nay = std::mem::take(&mut ep);
                // 3a. Các vùng
                if dang_dich && (co_moi || ep_lan_nay) {
                    for v in cfg_vung.iter().filter(|v| v.vung.is_some()) {
                        if !(ep_lan_nay || v.loai == "phu_de" || dem % VONG_CO_DINH == 0) {
                            continue;
                        }
                        let Some(anh) = may.lay(v.vung.unwrap()) else { continue };
                        let st = trang_thai.entry(v.id.clone()).or_default().clone();
                        let lam_dich = |chu: String, viec_dich: &mut HashMap<String, ViecNen<ViecDich>>| {
                            let st2 = st.clone();
                            let gh = gui_hien.clone();
                            let vd = viec_dich.entry(v.id.clone()).or_insert_with(|| ViecNen::moi(move |j| thuc_hien_dich(j, &st2, &gh)));
                            vd.gui(ViecDich {
                                v: v.clone(),
                                chu,
                                che_do: che_do.clone(),
                                giong,
                                thay_claude,
                                hien_google_truoc: google_truoc,
                            });
                        };
                        if v.loai == "phu_de" {
                            let nc = NguCanh { nguong, gui_hien: &gui_hien };
                            if let Some(chu) = xu_ly_vung(ocr, v, &anh, ep_lan_nay, &st, &nc) {
                                lam_dich(chu, &mut viec_dich);
                            }
                        } else {
                            // vùng cố định: đọc ở luồng riêng (OCR vùng lớn chậm)
                            let st2 = st.clone();
                            let gh = gui_hien.clone();
                            let (che_do2, giong2) = (che_do.clone(), giong);
                            let vd = viec_doc.entry(v.id.clone()).or_insert_with(move || {
                                let ocr2 = Ocr::moi().ok();
                                let mut viec: Option<ViecNen<ViecDich>> = None;
                                ViecNen::moi(move |(v, anh, ep): (Vung, Anh, bool)| {
                                    let Some(o) = ocr2.as_ref() else { return };
                                    let nc = NguCanh { nguong, gui_hien: &gh };
                                    if let Some(chu) = xu_ly_vung(o, &v, &anh, ep, &st2, &nc) {
                                        let (st3, gh3) = (st2.clone(), gh.clone());
                                        let vd = viec.get_or_insert_with(|| ViecNen::moi(move |j| thuc_hien_dich(j, &st3, &gh3)));
                                        vd.gui(ViecDich {
                                            v,
                                            chu,
                                            che_do: che_do2.clone(),
                                            giong: giong2,
                                            thay_claude: thay_co_dinh,
                                            hien_google_truoc: false,
                                        });
                                    }
                                })
                            });
                            vd.gui((v.clone(), anh, ep_lan_nay));
                        }
                    }
                }
                // 3b. Cả màn hình (F12)
                // màn hình đứng yên quá 5 giây vẫn quét lại (phòng DXGI ngừng gửi hình)
                let den_luot = quet_tm_cuoi.elapsed() >= Duration::from_millis(if co_moi || ep_lan_nay { 700 } else { 5000 });
                if toan_man && den_luot {
                    quet_tm_cuoi = Instant::now();
                    let (w, h) = may.kich_thuoc();
                    if let Some(anh) = may.lay([0, 0, w as i32, h as i32]) {
                        let bo: Vec<[i32; 4]> = if dang_dich { cfg_vung.iter().filter_map(|v| v.vung).collect() } else { vec![] };
                        let moi: Vec<Dong> = ocr::gom_doan(ocr.doc_toan_man(&anh))
                            .into_iter()
                            .filter(|d| ocr::hop_le_toan_man(&d.chu))
                            .filter(|d| !dich::the::la_tieng_viet(&d.chu) && !dich::the::la_viet_mat_dau(&d.chu))
                            .filter(|d| !bo.iter().any(|b| trong_vung(d.hop, *b)))
                            .collect();
                        let so_moi = moi.len();
                        cap_nhat_doan(&mut doan, moi, &anh, &gui_tm, &che_do);
                        let mut tt = chung.tt();
                        if tt.so_doan_toan_man != doan.len() {
                            nk(&format!("F12: đọc được {so_moi} đoạn, đang theo dõi {} đoạn", doan.len()));
                        }
                        tt.so_doan_toan_man = doan.len();
                    }
                }
            }
        } else if !(dang_dich || toan_man) {
            ql.tha();
        }

        // 4. Hiện khung dịch các vùng
        let an = chung.an_ban_dich.load(Ordering::SeqCst);
        let mut ds_khung = Vec::new();
        for v in &cfg_vung {
            let Some(k) = v.khung_dich() else { continue };
            let (chu, che) = match trang_thai.get(&v.id) {
                Some(st) if dang_dich && !an => {
                    let s = khoa(st);
                    (s.van_ban.clone(), s.che)
                }
                _ => (String::new(), None),
            };
            let kieu = match (che_goc, che, v.vung) {
                (true, Some((hop, Some((nen, mau)), cao)), Some(r)) if !chu.is_empty() => {
                    Kieu::Che { hop: [r[0] + hop[0] - 4, r[1] + hop[1] - 3, hop[2] + 8, hop[3] + 6], nen, chu: mau, cao }
                }
                _ if kieu_hien == "khung" => Kieu::Hop,
                _ => Kieu::PhuDe,
            };
            ds_khung.push(KhungHien {
                id: v.id.clone(),
                ten: v.ten.clone(),
                chu,
                khung: k,
                co_chu: v.co_chu,
                do_dam: if matches!(kieu, Kieu::Hop) { do_mo } else { do_dam_chu },
                kieu,
                can_giua: v.loai == "phu_de",
                vien,
            });
        }
        khung_dich::dat(ds_khung);

        // 5. Hiện F12
        let mut muc: Vec<MucPhu> = Vec::new();
        if toan_man && !an {
            for d in &doan {
                if let Some(vi) = d.vi.as_ref().filter(|vi| vi.trim().to_lowercase() != d.en.trim().to_lowercase()) {
                    let co = ((d.cao as f32) * 0.85) as i32;
                    let co = if co_tm > 0 { co.max(co_tm) } else { co.max(12) };
                    muc.push(MucPhu { hop: d.hop, chu: vi.clone(), co_chu: co, mau: if che_goc { d.mau } else { None } });
                }
            }
        }
        lop_phu::dat(muc);

        let chu_ky = Duration::from_millis((chu_ky.clamp(0.05, 1.0) * 1000.0) as u64);
        if !co_moi || !ql.may.as_ref().is_some_and(|m| m.ten() == "DXGI") {
            std::thread::sleep(chu_ky.saturating_sub(bat_dau.elapsed()).max(Duration::from_millis(15)));
        }
    }
}
