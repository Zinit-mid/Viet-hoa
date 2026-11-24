//! Tiến trình TOÀN CỤC: mỗi trang đăng ký một hàm `lay` trả về ảnh chụp tình hình việc nền của nó.
//! Giao diện đọc ở đây để vẽ: nút tiến độ (đổ màu theo %), khung "Việc đang chạy" ở cột phải, và dải dự phòng
//! ở đầu cửa sổ khi cửa sổ hẹp. Gọi `cap_nhat()` mỗi khung hình trước khi vẽ.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use eframe::egui::{self, Color32, RichText};

/// Tình hình một tác vụ tại thời điểm hỏi.
#[derive(Clone, Default)]
pub struct Anh {
    pub dang_ban: bool,
    pub ten: String,
    pub thong_bao: String,
    /// 0..1; None = chưa biết %
    pub tien: Option<f32>,
    pub loi: bool,
}

type Lay = Box<dyn Fn() -> Anh + Send + Sync>;

struct Muc {
    khoa: &'static str,
    lay: Lay,
    dung: Option<Arc<Mutex<Arc<AtomicBool>>>>,
    dang_ban: bool,
    bat_dau: Instant,
    anh: Anh,
    /// việc vừa xong: (thông báo, lỗi, lúc xong)
    xong: Option<(String, bool, Instant)>,
}

static DS: Mutex<Vec<Muc>> = Mutex::new(Vec::new());

/// Bọc một cờ dừng cố định thành kiểu dùng cho `dang_ky`.
pub fn co_dinh(c: Arc<AtomicBool>) -> Option<Arc<Mutex<Arc<AtomicBool>>>> {
    Some(Arc::new(Mutex::new(c)))
}

/// Đăng ký nguồn tiến trình. `dung`: cờ dừng hiện tại của tác vụ (nếu có).
pub fn dang_ky(khoa: &'static str, dung: Option<Arc<Mutex<Arc<AtomicBool>>>>, lay: impl Fn() -> Anh + Send + Sync + 'static) {
    let mut d = DS.lock().unwrap_or_else(|e| e.into_inner());
    if d.iter().any(|m| m.khoa == khoa) {
        return;
    }
    d.push(Muc { khoa, lay: Box::new(lay), dung, dang_ban: false, bat_dau: Instant::now(), anh: Anh::default(), xong: None });
}

/// Cập nhật trạng thái mọi nguồn (gọi mỗi khung hình).
pub fn cap_nhat() {
    let mut d = DS.lock().unwrap_or_else(|e| e.into_inner());
    for m in d.iter_mut() {
        let a = (m.lay)();
        if a.dang_ban && !m.dang_ban {
            m.bat_dau = Instant::now();
            m.xong = None;
        }
        if !a.dang_ban && m.dang_ban && !a.thong_bao.is_empty() {
            m.xong = Some((a.thong_bao.clone(), a.loi, Instant::now()));
        }
        m.dang_ban = a.dang_ban;
        m.anh = a;
        if m.xong.as_ref().is_some_and(|x| x.2.elapsed().as_secs() > 60) {
            m.xong = None;
        }
    }
}

/// Tình hình của một nguồn (cho nút tiến độ).
pub struct TinhHinh {
    pub dang_ban: bool,
    pub tien: Option<f32>,
    pub thong_bao: String,
    pub giay: u64,
    pub xong: Option<(String, bool, Instant)>,
}

pub fn tinh_hinh(khoa: &str) -> Option<TinhHinh> {
    let d = DS.lock().unwrap_or_else(|e| e.into_inner());
    d.iter().find(|m| m.khoa == khoa).map(|m| TinhHinh {
        dang_ban: m.dang_ban,
        tien: m.anh.tien,
        thong_bao: m.anh.thong_bao.clone(),
        giay: m.bat_dau.elapsed().as_secs(),
        xong: m.xong.clone(),
    })
}

/// Yêu cầu dừng việc của một nguồn.
pub fn dung_viec(khoa: &str) {
    let d = DS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = d.iter().find(|m| m.khoa == khoa) {
        if let Some(c) = &m.dung {
            c.lock().unwrap_or_else(|e| e.into_inner()).store(true, Ordering::SeqCst);
        }
    }
}

/// Các việc đang chạy: (khóa, ảnh, giây đã chạy).
pub fn dang_chay() -> Vec<(&'static str, Anh, u64)> {
    let d = DS.lock().unwrap_or_else(|e| e.into_inner());
    d.iter().filter(|m| m.dang_ban).map(|m| (m.khoa, m.anh.clone(), m.bat_dau.elapsed().as_secs())).collect()
}

/// Việc vừa xong gần đây (còn hiện): (khóa, thông báo, lỗi).
pub fn vua_xong() -> Vec<(&'static str, String, bool)> {
    let d = DS.lock().unwrap_or_else(|e| e.into_inner());
    d.iter().filter_map(|m| m.xong.as_ref().filter(|_| !m.dang_ban).map(|x| (m.khoa, x.0.clone(), x.1))).collect()
}

pub fn tat_xong(khoa: &str) {
    let mut d = DS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = d.iter_mut().find(|m| m.khoa == khoa) {
        m.xong = None;
    }
}

pub fn dung_mm(giay: u64) -> String {
    format!("{}:{:02}", giay / 60, giay % 60)
}

/// Dải dự phòng ở đầu cửa sổ (dùng khi cửa sổ quá hẹp để hiện cột phải).
pub fn ve_dai_dau(ctx: &egui::Context) {
    let dang = dang_chay();
    let xong = vua_xong();
    if dang.is_empty() && xong.is_empty() {
        return;
    }
    egui::TopBottomPanel::top("tac_vu").frame(egui::Frame::NONE.fill(Color32::from_rgb(20, 20, 44)).inner_margin(8.0)).show(ctx, |ui| {
        for (k, a, giay) in &dang {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(crate::kieu::dam(if a.ten.is_empty() { "Đang làm việc" } else { &a.ten }));
                ui.label(RichText::new(format!("• {}", dung_mm(*giay))).weak());
                if ui.small_button("Dừng").clicked() {
                    dung_viec(k);
                }
            });
            let thanh = match a.tien {
                Some(t) => egui::ProgressBar::new(t.clamp(0.0, 1.0)).show_percentage(),
                None => egui::ProgressBar::new(0.0).text("đang chạy… (chưa biết %)"),
            };
            ui.add(thanh.desired_height(18.0));
            if !a.thong_bao.is_empty() {
                ui.add(egui::Label::new(RichText::new(&a.thong_bao).small()).truncate());
            }
        }
        let mut tat = Vec::new();
        for (k, tb, loi) in &xong {
            ui.horizontal(|ui| {
                let m = if *loi { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN };
                ui.label(RichText::new(if *loi { "✘" } else { "✔" }).color(m));
                ui.add(egui::Label::new(RichText::new(tb).color(m)).truncate());
                if ui.small_button("Ẩn").clicked() {
                    tat.push(*k);
                }
            });
        }
        for k in tat {
            tat_xong(k);
        }
    });
}
