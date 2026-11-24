//! Thanh tiến trình TOÀN CỤC: luôn hiện ở đầu cửa sổ khi có tác vụ đang chạy (ở bất kỳ trang nào),
//! cho biết tên việc, % hoàn thành (hoặc thanh chạy + thời gian đã chạy nếu chưa biết %), thông báo hiện tại,
//! nút Dừng; xong thì báo kết quả vài chục giây. Mỗi trang đăng ký một hàm `lay` trả về ảnh chụp tình hình.

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
    d.push(Muc { khoa, lay: Box::new(lay), dung, dang_ban: false, bat_dau: Instant::now(), xong: None });
}

fn dung_mm(giay: u64) -> String {
    format!("{}:{:02}", giay / 60, giay % 60)
}

/// Vẽ thanh tiến trình toàn cục (gọi mỗi khung hình). Chỉ chiếm chỗ khi có việc / vừa xong.
pub fn ve(ctx: &egui::Context) {
    let mut d = DS.lock().unwrap_or_else(|e| e.into_inner());
    let mut co_ban = false;
    // cập nhật trạng thái
    let anh: Vec<Anh> = d.iter().map(|m| (m.lay)()).collect();
    for (m, a) in d.iter_mut().zip(&anh) {
        if a.dang_ban && !m.dang_ban {
            m.bat_dau = Instant::now();
            m.xong = None;
        }
        if !a.dang_ban && m.dang_ban && !a.thong_bao.is_empty() {
            m.xong = Some((a.thong_bao.clone(), a.loi, Instant::now()));
        }
        m.dang_ban = a.dang_ban;
        co_ban |= a.dang_ban;
        if m.xong.as_ref().is_some_and(|x| x.2.elapsed().as_secs() > 60) {
            m.xong = None;
        }
    }
    let co_xong = d.iter().any(|m| m.xong.is_some());
    if !co_ban && !co_xong {
        return;
    }
    egui::TopBottomPanel::top("tac_vu").show(ctx, |ui| {
        for (m, a) in d.iter_mut().zip(&anh) {
            if a.dang_ban {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(if a.ten.is_empty() { "Đang làm việc" } else { &a.ten }).strong());
                    ui.label(RichText::new(format!("• {}", dung_mm(m.bat_dau.elapsed().as_secs()))).weak());
                    if let Some(dung) = &m.dung {
                        if ui.small_button("Dừng").clicked() {
                            dung.lock().unwrap_or_else(|e| e.into_inner()).store(true, Ordering::SeqCst);
                        }
                    }
                });
                let thanh = match a.tien {
                    Some(t) => egui::ProgressBar::new(t.clamp(0.0, 1.0)).show_percentage(),
                    None => egui::ProgressBar::new(0.0).animate(true).text("đang chạy… (chưa biết %)"),
                };
                ui.add(thanh.desired_height(18.0));
                if !a.thong_bao.is_empty() {
                    ui.add(egui::Label::new(RichText::new(&a.thong_bao).small()).truncate());
                }
            } else if let Some((tb, loi, _)) = &m.xong {
                let (tb, loi) = (tb.clone(), *loi);
                let mut tat = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if loi { "✘" } else { "✔" }).color(if loi { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN }));
                    ui.add(egui::Label::new(RichText::new(tb).color(if loi { Color32::LIGHT_RED } else { Color32::LIGHT_GREEN })).truncate());
                    if ui.small_button("Ẩn").clicked() {
                        tat = true;
                    }
                });
                if tat {
                    m.xong = None;
                }
            }
        }
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}
