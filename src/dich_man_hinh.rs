//! Vòng dịch màn hình: vùng (phu_de / co_dinh) + F12 dịch cả màn hình.
//!
//! - Chỉ đọc khi màn hình có hình mới, chỉ đọc phần vùng cần.
//! - Mỗi vùng có một hàng dịch "chỉ giữ câu mới nhất": không mất câu, không dồn việc.
//! - Vị trí, màu che, cỡ chữ đi kèm đúng câu được dịch.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::cau_hinh::{CauHinh, Vung, SO_VUNG_TOI_DA};
use crate::chup::{self, Anh};
use crate::lop_phu::{self, MucPhu, PhimTat};
use crate::ocr::{self, Dong, Ocr};
use crate::{che_mau, dich, giong_doc};

/// Lệnh gửi vào luồng dịch (từ giao diện hoặc phím tắt).
#[derive(Debug)]
pub enum Lenh {
    Phim(PhimTat),
    /// Đã chọn xong vùng mới (chỉ số, toạ độ)
    VungMoi(usize, [i32; 4]),
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
    pub dang_dich: AtomicBool,
    pub toan_man: AtomicBool,
    pub dang_chon_vung: AtomicBool,
    /// Giao diện cần lưu cấu hình (vùng vừa đổi)
    pub can_luu: AtomicBool,
}

impl Chung {
    pub fn moi(ch: CauHinh) -> Arc<Self> {
        Arc::new(Self {
            cau_hinh: Mutex::new(ch),
            tinh_trang: Mutex::new(TinhTrang::default()),
            dang_dich: AtomicBool::new(false),
            toan_man: AtomicBool::new(false),
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

/// Mở khung chọn vùng ở luồng riêng; kết quả gửi về luồng dịch.
pub fn chon_vung(chung: &Arc<Chung>, gui: &Sender<Lenh>, chi_so: usize) {
    if chung.dang_chon_vung.swap(true, Ordering::SeqCst) {
        return;
    }
    let (chung, gui) = (chung.clone(), gui.clone());
    std::thread::spawn(move || {
        if let Some(v) = lop_phu::chon_vung() {
            let _ = gui.send(Lenh::VungMoi(chi_so, v));
        }
        chung.dang_chon_vung.store(false, Ordering::SeqCst);
    });
}

fn nap_san_offline(chung: &Chung) {
    if chung.ch().che_do_hoi_thoai.starts_with("offline") {
        dich::offline::nap_san();
    }
}

// ---------------- hàng dịch "chỉ giữ câu mới nhất" ----------------

#[derive(Clone)]
struct YeuCau {
    en: String,
    /// khung hiện chữ (toạ độ màn hình) + màu + cỡ chữ của đúng câu này
    hop: [i32; 4],
    mau: Option<([u8; 3], [u8; 3])>,
    co: i32,
    che_do: String,
    /// câu đã đứng yên → cho Claude dịch ngầm để lưu
    luu_claude: bool,
    luc: Instant,
}

struct KetQuaVung {
    id: String,
    yc: YeuCau,
    vi: String,
    may: dich::May,
}

struct Khe {
    cho: Mutex<Option<YeuCau>>,
    bao: Condvar,
}

impl Khe {
    fn moi(id: String, gui: Sender<KetQuaVung>) -> Arc<Khe> {
        let khe = Arc::new(Khe { cho: Mutex::new(None), bao: Condvar::new() });
        let k = khe.clone();
        std::thread::spawn(move || loop {
            let yc = {
                let mut g = k.cho.lock().unwrap_or_else(|e| e.into_inner());
                while g.is_none() {
                    g = k.bao.wait(g).unwrap_or_else(|e| e.into_inner());
                }
                g.take().unwrap()
            };
            let kq = dich::dich_nhanh(&yc.en, &yc.che_do);
            if kq.is_none() {
                nk(&format!("Không dịch được: {}", yc.en));
            }
            if let Some((vi, may)) = kq {
                if yc.luu_claude && yc.che_do == "offline_claude" && may != dich::May::BoNho {
                    dich::claude_luu_ngam(yc.en.clone());
                }
                if gui.send(KetQuaVung { id: id.clone(), yc, vi, may }).is_err() {
                    return;
                }
            }
        });
        khe
    }

    /// Đặt câu mới; câu cũ chưa kịp dịch bị thay (không dồn việc).
    fn dat(&self, yc: YeuCau) {
        *self.cho.lock().unwrap_or_else(|e| e.into_inner()) = Some(yc);
        self.bao.notify_one();
    }
}

// ---------------- trạng thái ----------------

#[derive(Default)]
struct TrangVung {
    chu: String,
    on_dinh: u32,
    da_gui: String,
    da_gui_luu: bool,
    trong: u32,
    co: i32,
    hien: Option<MucPhu>,
}

struct Doan {
    chu: String,
    hop: [i32; 4],
    co: i32,
    lan_thay: u32,
    mat: u32,
    vi: Option<String>,
    da_claude: bool,
    mau: Option<([u8; 3], [u8; 3])>,
}

// ---------------- quản lý bộ chụp ----------------

/// Giữ bộ chụp; mất quyền chụp thì bỏ hẳn bộ cũ, chờ dần (1 s, 2 s, 5 s) rồi mới tạo lại.
/// Lỗi 5 lần liền → chụp GDI, cứ 30 s thử lại DXGI một lần.
#[derive(Default)]
struct QuanLyChup {
    may: Option<Box<dyn chup::MayChup + Send>>,
    loi_lien: u32,
    cho_den: Option<Instant>,
    gdi_tu: Option<Instant>,
    /// Hàm tạo bộ chụp nhanh (để kiểm thử thay bằng bộ giả)
    tao_nhanh: Option<fn() -> anyhow::Result<Box<dyn chup::MayChup + Send>>>,
}

impl QuanLyChup {
    fn tha(&mut self) {
        if self.may.take().is_some() {
            nk("Thả bộ chụp (không dịch)");
        }
        self.loi_lien = 0;
        self.cho_den = None;
        self.gdi_tu = None;
    }

    fn lay_may(&mut self, chung: &Chung) -> Option<&mut Box<dyn chup::MayChup + Send>> {
        // Đang dùng GDI quá 30 s → thử lại DXGI
        if let Some(t) = self.gdi_tu {
            if t.elapsed() >= Duration::from_secs(30) {
                self.may = None;
                self.gdi_tu = None;
                self.loi_lien = 4; // lỗi thêm 1 lần là quay lại GDI ngay
            }
        }
        if self.may.is_none() {
            if self.cho_den.is_some_and(|t| Instant::now() < t) {
                return None;
            }
            if self.gdi_tu.is_some() {
                self.may = Some(Box::new(chup::ChupGdi));
            } else {
                let tao = self.tao_nhanh.unwrap_or(|| Ok(Box::new(chup::ChupDxgi::moi()?)));
                match tao() {
                    Ok(m) => self.may = Some(m),
                    Err(e) => {
                        self.loi(chung, &format!("không tạo được DXGI: {e}"));
                        return self.may.as_mut();
                    }
                }
            }
            let ten = self.may.as_ref().map(|m| m.ten()).unwrap_or("");
            nk(&format!("Chụp bằng {ten}"));
            chung.tt().may_chup = ten.to_string();
        }
        self.may.as_mut()
    }

    fn thanh_cong(&mut self, chung: &Chung) {
        if self.loi_lien > 0 && self.gdi_tu.is_none() {
            nk("Chụp lại được bình thường");
            chung.tt().loi.clear();
        }
        if self.gdi_tu.is_none() {
            self.loi_lien = 0;
        }
    }

    fn loi(&mut self, chung: &Chung, e: &str) {
        self.may = None; // bỏ hẳn bộ cũ trước khi tạo lại
        self.loi_lien += 1;
        if self.loi_lien == 1 {
            nk(&format!("Chụp lỗi: {e} – sẽ thử lại"));
        }
        if self.loi_lien >= 5 {
            if self.gdi_tu.is_none() {
                nk("Lỗi chụp 5 lần liền → chuyển sang chụp GDI, 30 giây sau thử lại DXGI");
            }
            self.gdi_tu = Some(Instant::now());
            self.cho_den = None;
            chung.tt().loi = "Chụp nhanh (DXGI) đang lỗi → tạm dùng GDI. Game nên để Borderless / Windowed.".into();
            return;
        }
        let cho = match self.loi_lien {
            1 => 1,
            2 => 2,
            _ => 5,
        };
        self.cho_den = Some(Instant::now() + Duration::from_secs(cho));
        chung.tt().loi = format!("Chụp lỗi: {e} – thử lại sau {cho} giây");
    }
}

fn nk(s: &str) {
    if !cfg!(test) {
        crate::duong_dan::nhat_ky(s);
    }
}

/// Chữ nhỏ hơn mức này ở F12 bỏ qua (chữ dưới icon, chữ ghi chú nhỏ)
const CAO_CHU_TOI_THIEU: i32 = 12;

fn chuan_hoa(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Bản dịch gần như y hệt bản gốc (chữ rác OCR, tên riêng) → không cần hiện khung.
fn giong_ban_goc(en: &str, vi: &str) -> bool {
    let (a, b) = (chuan_hoa(en), chuan_hoa(vi));
    a == b || strsim::normalized_levenshtein(&a, &b) >= 0.85
}

/// Tỉ lệ chồng nhau của 2 khung (phần giao / khung nhỏ hơn).
fn chong(a: [i32; 4], b: [i32; 4]) -> f32 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    if w <= 0 || h <= 0 {
        return 0.0;
    }
    let nho = (a[2] * a[3]).min(b[2] * b[3]).max(1);
    (w * h) as f32 / nho as f32
}

fn het_cau(s: &str) -> bool {
    s.trim_end().ends_with(['.', '!', '?', '"', '…'])
}

fn giong_nhau(a: &str, b: &str) -> bool {
    a == b || strsim::normalized_levenshtein(a, b) >= 0.9
}

/// Cỡ chữ Việt theo chiều cao dòng chữ gốc; chỉ đổi khi lệch > 25% (không nhảy).
fn co_on_dinh(cu: i32, cao_dong: i32) -> i32 {
    let moi = ((cao_dong as f32) * 0.72).round() as i32;
    let moi = moi.clamp(12, 48);
    if cu == 0 || (moi - cu).abs() * 4 > cu {
        moi
    } else {
        cu
    }
}

pub fn chay(chung: Arc<Chung>, nhan: Receiver<Lenh>, gui: Sender<Lenh>) {
    let _ = unsafe { windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED) };
    let mut ql = QuanLyChup::default();
    nk("Khởi động luồng dịch");
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
    let (gui_kq, nhan_kq) = channel::<KetQuaVung>();
    let (gui_tm, nhan_tm) = channel::<(Vec<String>, Vec<Option<String>>)>();
    let mut khe: HashMap<String, Arc<Khe>> = HashMap::new();
    let mut vung: HashMap<String, TrangVung> = HashMap::new();
    let mut doan: Vec<Doan> = Vec::new();
    let mut vong: u64 = 0;
    let mut quet_tm_cuoi = Instant::now() - Duration::from_secs(5);
    let mut can_doc_lai = true;
    let mut hoat_dong = false;

    loop {
        // 1. Lệnh
        while let Ok(l) = nhan.try_recv() {
            nk(&format!("Lệnh: {l:?}"));
            match l {
                Lenh::Phim(PhimTat::ChonVung(i)) => chon_vung(&chung, &gui, i),
                Lenh::Phim(PhimTat::ToanMan) => {
                    let b = !chung.toan_man.load(Ordering::SeqCst);
                    chung.toan_man.store(b, Ordering::SeqCst);
                    doan.clear();
                    can_doc_lai = true;
                    if b {
                        nap_san_offline(&chung);
                    }
                }
                Lenh::Phim(PhimTat::BatTatDich) => {
                    let b = !chung.dang_dich.load(Ordering::SeqCst);
                    chung.dang_dich.store(b, Ordering::SeqCst);
                    vung.clear();
                    can_doc_lai = true;
                    if b {
                        nap_san_offline(&chung);
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
                Lenh::VungMoi(i, v) => {
                    {
                        let mut ch = chung.ch();
                        if i < ch.cac_vung.len() {
                            ch.cac_vung[i].vung = v;
                            vung.remove(&ch.cac_vung[i].id);
                        } else if ch.cac_vung.len() < SO_VUNG_TOI_DA {
                            let so = (1..).find(|n| !ch.cac_vung.iter().any(|v| v.id == format!("vung_{n}"))).unwrap_or(1);
                            let id = format!("vung_{so}");
                            ch.cac_vung.push(Vung { id, ten: format!("Vùng {so}"), vung: v, ..Default::default() });
                        }
                    }
                    chung.can_luu.store(true, Ordering::SeqCst);
                    chung.dang_dich.store(true, Ordering::SeqCst);
                    can_doc_lai = true;
                    nap_san_offline(&chung);
                }
            }
        }

        // 2. Kết quả dịch vùng: hiện đúng khung / màu / cỡ của câu đó
        while let Ok(kq) = nhan_kq.try_recv() {
            let Some(st) = vung.get_mut(&kq.id) else { continue };
            if st.chu.is_empty() {
                continue; // vùng đã trống, bỏ bản dịch muộn
            }
            let moi = st.hien.as_ref().map(|h| h.chu != kq.vi).unwrap_or(true);
            st.hien = Some(MucPhu { hop: kq.yc.hop, chu: kq.vi.clone(), co_chu: kq.yc.co, mau: kq.yc.mau });
            let ch = chung.ch();
            if moi && ch.giong_doc && kq.yc.luu_claude && ch.cac_vung.iter().any(|v| v.id == kq.id && v.loai == "phu_de") {
                giong_doc::doc(kq.vi.clone(), ch.giong.clone());
            }
            drop(ch);
            nk(&format!("Dịch xong ({}, {} ms): {} => {}", kq.may.ten(), kq.yc.luc.elapsed().as_millis(), kq.yc.en, kq.vi));
            let mut tt = chung.tt();
            tt.cuoi_ms = kq.yc.luc.elapsed().as_millis();
            tt.cuoi_en = kq.yc.en;
            tt.cuoi_vi = kq.vi;
            tt.cuoi_may = kq.may.ten().into();
        }
        while let Ok((en, vi)) = nhan_tm.try_recv() {
            for (e, v) in en.iter().zip(vi) {
                if let (Some(d), Some(v)) = (doan.iter_mut().find(|d| &d.chu == e), v) {
                    nk(&format!("F12 dịch: {e} => {v}"));
                    d.vi = Some(v);
                }
            }
        }

        let dang_dich = chung.dang_dich.load(Ordering::SeqCst);
        let toan_man = chung.toan_man.load(Ordering::SeqCst);
        if (dang_dich || toan_man) != hoat_dong {
            hoat_dong = dang_dich || toan_man;
            lop_phu::bat_che_do_dich(hoat_dong);
            nk(if hoat_dong { "Bắt đầu dịch: bật lớp nền, ẩn bảng app khỏi ảnh chụp" } else { "Dừng dịch" });
        }
        let Some(ocr) = ocr.as_ref().filter(|_| (dang_dich || toan_man) && !chung.dang_chon_vung.load(Ordering::SeqCst)) else {
            // Không dịch: thả bộ chụp để mở / tắt game không bị ảnh hưởng
            ql.tha();
            lop_phu::dat(Vec::new());
            std::thread::sleep(Duration::from_millis(120));
            continue;
        };

        // 3. Chụp: chỉ đọc lại khi màn hình có hình mới
        let bat_dau = Instant::now();
        let kq_chup = match ql.lay_may(&chung) {
            Some(m) => m.cap_nhat(),
            None => {
                lop_phu::dat(Vec::new());
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        let co_moi = match kq_chup {
            Ok(b) => {
                ql.thanh_cong(&chung);
                b
            }
            Err(e) => {
                ql.loi(&chung, &e.to_string());
                continue;
            }
        };
        let Some(may) = ql.may.as_mut() else { continue };
        let la_dxgi = may.ten() == "DXGI";
        let doc = co_moi || std::mem::take(&mut can_doc_lai);
        vong += 1;
        // Áp lại "ẩn bảng app khỏi ảnh chụp" định kỳ (cửa sổ app có thể tạo sau)
        if vong % 20 == 1 {
            lop_phu::bat_che_do_dich(true);
        }

        let (cac_vung, che, che_do, co_tm) = {
            let ch = chung.ch();
            (ch.cac_vung.clone(), ch.che_chu_goc, ch.che_do_hoi_thoai.clone(), ch.co_chu_toan_man)
        };
        let mut muc: Vec<MucPhu> = Vec::new();

        if dang_dich {
            for v in &cac_vung {
                let st = vung.entry(v.id.clone()).or_default();
                let den_luot = v.loai != "co_dinh" || vong % 5 == 1;
                if doc && den_luot {
                    if let Some(anh) = may.lay(v.vung) {
                        let k = khe.entry(v.id.clone()).or_insert_with(|| Khe::moi(v.id.clone(), gui_kq.clone()));
                        xu_ly_vung(ocr, &anh, v, st, che, &che_do, k);
                    }
                }
                if let Some(h) = st.hien.as_ref().filter(|h| !giong_ban_goc(&st.chu, &h.chu)) {
                    let mut h = h.clone();
                    if !che {
                        h.mau = None;
                    }
                    muc.push(h);
                }
            }
        }

        // 4. Cả màn hình (F12)
        if toan_man {
            if doc && quet_tm_cuoi.elapsed() >= Duration::from_millis(700) {
                quet_tm_cuoi = Instant::now();
                let (w, h) = may.kich_thuoc();
                if let Some(anh) = may.lay([0, 0, w as i32, h as i32]) {
                    let bo: Vec<[i32; 4]> = if dang_dich { cac_vung.iter().map(|v| v.vung).collect() } else { vec![] };
                    let dong: Vec<Dong> = ocr::gom_doan(ocr.doc_toan_man(&anh))
                        .into_iter()
                        .filter(|d| d.cao_dong >= CAO_CHU_TOI_THIEU)
                        .filter(|d| !dich::the::la_tieng_viet(&d.chu) && !dich::the::la_viet_mat_dau(&d.chu))
                        .filter(|d| !bo.iter().any(|b| trong_vung(d.hop, *b)))
                        .collect();
                    cap_nhat_doan(&mut doan, dong, &anh, &gui_tm, &che_do);
                    chung.tt().so_doan_toan_man = doan.len();
                }
            }
            for d in &doan {
                if let Some(vi) = d.vi.as_ref().filter(|vi| !giong_ban_goc(&d.chu, vi)) {
                    let co = if co_tm > 0 { co_tm.max(d.co) } else { d.co };
                    muc.push(MucPhu { hop: d.hop, chu: vi.clone(), co_chu: co, mau: if che { d.mau } else { None } });
                }
            }
        }

        lop_phu::dat(muc);
        // DXGI đã chờ tối đa 100 ms trong cap_nhat; GDI thì nghỉ cho nhẹ máy
        if !la_dxgi || !co_moi {
            let ngu = Duration::from_millis(120).saturating_sub(bat_dau.elapsed());
            std::thread::sleep(ngu.max(Duration::from_millis(15)));
        }
    }
}

fn trong_vung(h: [i32; 4], v: [i32; 4]) -> bool {
    let (cx, cy) = (h[0] + h[2] / 2, h[1] + h[3] / 2);
    cx >= v[0] && cx <= v[0] + v[2] && cy >= v[1] && cy <= v[1] + v[3]
}

fn xu_ly_vung(ocr: &Ocr, anh: &Anh, v: &Vung, st: &mut TrangVung, che: bool, che_do: &str, khe: &Khe) {
    let mut doc_anh = anh.clone();
    if v.loc_trang {
        doc_anh.loc_trang();
    }
    let dong: Vec<Dong> =
        ocr.doc(&doc_anh).unwrap_or_default().into_iter().filter(|d| ocr::dong_hop_le(&d.chu)).collect();
    let chu = dong.iter().map(|d| d.chu.as_str()).collect::<Vec<_>>().join(" ");
    let an_sau = if v.loai == "co_dinh" { 8 } else { 3 };
    if chu.trim().is_empty() || dich::the::la_tieng_viet(&chu) {
        st.trong += 1;
        if st.trong >= an_sau {
            st.hien = None;
            st.chu.clear();
            st.da_gui.clear();
            st.da_gui_luu = false;
        }
        return;
    }
    st.trong = 0;
    if giong_nhau(&chu, &st.chu) {
        st.on_dinh += 1;
    } else {
        st.on_dinh = 1;
    }
    if st.on_dinh == 1 {
        nk(&format!("OCR [{}]: {chu}", v.ten));
    }
    st.chu = chu;

    let dung_yen = het_cau(&st.chu) || st.on_dinh >= 2;
    let can_gui = (st.chu != st.da_gui && (dung_yen || st.chu.chars().count() >= 30))
        || (dung_yen && !st.da_gui_luu);
    if !can_gui {
        return;
    }

    // Khung = hộp hợp các dòng OCR (toạ độ trong ảnh vùng → màn hình)
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let mut cao: Vec<i32> = Vec::new();
    for d in &dong {
        x0 = x0.min(d.hop[0]);
        y0 = y0.min(d.hop[1]);
        x1 = x1.max(d.hop[0] + d.hop[2]);
        y1 = y1.max(d.hop[1] + d.hop[3]);
        cao.push(d.cao_dong);
    }
    cao.sort_unstable();
    let cao_dong = cao.get(cao.len() / 2).copied().unwrap_or(20);
    let co = if v.co_chu_tu_dong {
        st.co = co_on_dinh(st.co, cao_dong);
        st.co
    } else {
        v.co_chu
    };
    let hop_anh = [x0 - 4, y0 - 3, x1 - x0 + 8, y1 - y0 + 6];
    let (hop, mau) = if che {
        let mau = che_mau::mau_che(anh, hop_anh);
        ([anh.x0 + hop_anh[0], anh.y0 + hop_anh[1], hop_anh[2], hop_anh[3]], mau)
    } else {
        (v.vung, None)
    };

    nk(&format!("Gửi dịch [{}] cỡ {co} khung {hop:?} màu {mau:?}", v.ten));
    st.da_gui = st.chu.clone();
    st.da_gui_luu = dung_yen;
    khe.dat(YeuCau {
        en: st.chu.clone(),
        hop,
        mau,
        co,
        che_do: che_do.to_string(),
        luu_claude: dung_yen,
        luc: Instant::now(),
    });
}

fn cap_nhat_doan(
    doan: &mut Vec<Doan>,
    moi: Vec<Dong>,
    anh: &Anh,
    gui_tm: &Sender<(Vec<String>, Vec<Option<String>>)>,
    che_do: &str,
) {
    let mut da_khop = vec![false; doan.len()];
    let mut them: Vec<Doan> = Vec::new();
    let mut can_nhanh: Vec<String> = Vec::new();
    for d in moi {
        let hop_anh = [d.hop[0] - anh.x0, d.hop[1] - anh.y0, d.hop[2], d.hop[3]];
        // Cùng chỗ (chồng ≥ 50%) = cùng đoạn, dù OCR đọc lệch vài chữ
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
                let giong = p.chu == d.chu || strsim::normalized_levenshtein(&p.chu, &d.chu) >= 0.75;
                if !giong {
                    // chữ đổi hẳn ở cùng chỗ → dịch lại, giữ khung cũ tới khi có bản mới
                    nk(&format!("F12 đoạn đổi {:?}: {} -> {}", d.hop, p.chu, d.chu));
                    p.chu = d.chu;
                    p.hop = d.hop;
                    p.co = co_on_dinh(p.co, d.cao_dong);
                    p.mau = che_mau::mau_che(anh, hop_anh);
                    p.lan_thay = 1;
                    p.da_claude = false;
                    match dich::tra_bo_nho(&p.chu) {
                        Some(v) => p.vi = Some(v),
                        None => can_nhanh.push(p.chu.clone()),
                    }
                } else if (p.hop[0] - d.hop[0]).abs() > 8 || (p.hop[1] - d.hop[1]).abs() > 8 {
                    p.hop = d.hop; // chỉ dời khi lệch thật
                }
            }
            None => {
                nk(&format!("F12 đoạn mới {:?}: {}", d.hop, d.chu));
                let vi = dich::tra_bo_nho(&d.chu);
                if vi.is_none() {
                    can_nhanh.push(d.chu.clone());
                }
                them.push(Doan {
                    mau: che_mau::mau_che(anh, hop_anh),
                    vi,
                    co: co_on_dinh(0, d.cao_dong),
                    chu: d.chu,
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
    // Mất 3 lần quét liền mới xóa
    doan.retain(|p| p.mat < 3);
    doan.extend(them);
    // Đoạn mới / đổi: hiện ngay bằng Google (hoặc offline nếu chọn offline)
    if !can_nhanh.is_empty() {
        let gui = gui_tm.clone();
        let chi_offline = che_do == "offline";
        std::thread::spawn(move || {
            let vi: Vec<Option<String>> = if chi_offline {
                dich::dich_nhieu(&can_nhanh, false)
            } else {
                can_nhanh.iter().map(|c| dich::google::dich(c).ok()).collect()
            };
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
                p.chu.clone()
            })
            .collect();
        if !lo.is_empty() {
            let gui = gui_tm.clone();
            std::thread::spawn(move || {
                let vi = dich::dich_nhieu(&lo, true);
                let _ = gui.send((lo, vi));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Hong;
    impl chup::MayChup for Hong {
        fn cap_nhat(&mut self) -> anyhow::Result<bool> {
            Err(anyhow::anyhow!("DXGI mất quyền chụp"))
        }
        fn lay(&mut self, _: [i32; 4]) -> Option<Anh> {
            None
        }
        fn kich_thuoc(&self) -> (u32, u32) {
            (0, 0)
        }
        fn ten(&self) -> &'static str {
            "DXGI"
        }
    }

    #[test]
    fn mat_quyen_chup_cho_dan_roi_sang_gdi() {
        let chung = Chung::moi(CauHinh::default());
        let mut ql = QuanLyChup { tao_nhanh: Some(|| Ok(Box::new(Hong))), ..Default::default() };
        let mut so_lan_tao = 0;
        let mut cho = Vec::new();
        for _ in 0..5 {
            // bỏ qua thời gian chờ cho nhanh
            if let Some(t) = ql.cho_den {
                cho.push((t - Instant::now()).as_secs_f32().round() as u32);
                ql.cho_den = Some(Instant::now());
            }
            let m = ql.lay_may(&chung).expect("phải tạo được bộ chụp");
            so_lan_tao += 1;
            let e = m.cap_nhat().unwrap_err();
            ql.loi(&chung, &e.to_string());
            assert!(ql.may.is_none(), "bộ cũ phải bị bỏ hẳn");
        }
        assert_eq!(so_lan_tao, 5);
        assert_eq!(cho, vec![1, 2, 5, 5]);
        // Trong lúc chờ không được tạo bộ chụp mới
        let mut ql2 = QuanLyChup { tao_nhanh: Some(|| Ok(Box::new(Hong))), ..Default::default() };
        ql2.lay_may(&chung);
        ql2.loi(&chung, "x");
        assert!(ql2.lay_may(&chung).is_none());
        // Sau 5 lần: dùng GDI
        assert!(ql.gdi_tu.is_some());
        assert_eq!(ql.lay_may(&chung).unwrap().ten(), "GDI");
        // 30 giây sau thử lại DXGI; hỏng thêm 1 lần là quay lại GDI ngay
        ql.gdi_tu = Some(Instant::now() - Duration::from_secs(31));
        assert_eq!(ql.lay_may(&chung).unwrap().ten(), "DXGI");
        let e = ql.may.as_mut().unwrap().cap_nhat().unwrap_err();
        ql.loi(&chung, &e.to_string());
        assert_eq!(ql.lay_may(&chung).unwrap().ten(), "GDI");
        // Không dịch: thả bộ chụp
        ql.tha();
        assert!(ql.may.is_none() && ql.gdi_tu.is_none());
    }
}
