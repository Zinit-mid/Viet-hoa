//! Hộp thoại "Thêm giọng mẫu" dùng chung (Lồng tiếng, Thuyết minh phim, Clone giọng...):
//! chọn file ghi âm (wav / mp3 / m4a / mp4...) + đoạn giây → xử lý (cắt, bỏ lặng, mono 24 kHz, lọc 70 Hz,
//! chuẩn hóa độ to) → lưu vào kho giọng → (tùy chọn) gán luôn cho một nhân vật của game.
//! Giọng trong kho dùng được cho lồng tiếng game, thuyết minh phim và mọi nơi chọn giọng.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::giong_doc;
use crate::viet_hoa::clone_giong as cg;

/// Tăng mỗi lần lưu được một giọng mới – các trang so sánh để nạp lại danh sách kho.
pub static SO_LAN_LUU: AtomicUsize = AtomicUsize::new(0);

const PHAN: [(&str, &str); 4] = [
    ("", "không gán"),
    ("A", "Crimson Desert – nhân vật có tên (A)"),
    ("B", "Crimson Desert – dân thường (B)"),
    ("TR2013", "Tomb Raider (2013)"),
];

#[derive(Default)]
struct Tt {
    tb: String,
    loi: bool,
    ban: bool,
    mau: Option<Vec<f32>>,
}

pub struct ThemGiong {
    pub mo: bool,
    nguon: String,
    tu: f64,
    den: f64,
    ten: String,
    ghi_de: bool,
    phan: usize,
    nguoi: String,
    tt: Arc<Mutex<Tt>>,
}

impl Default for ThemGiong {
    fn default() -> Self {
        Self {
            mo: false,
            nguon: String::new(),
            tu: 0.0,
            den: 0.0,
            ten: String::new(),
            ghi_de: false,
            phan: 0,
            nguoi: String::new(),
            tt: Arc::new(Mutex::new(Tt::default())),
        }
    }
}

impl ThemGiong {
    /// Nút "+ Thêm giọng mẫu"; bấm thì mở cửa sổ.
    pub fn nut(&mut self, ui: &mut egui::Ui) {
        if ui.button("+ Thêm giọng mẫu").on_hover_text("Thêm giọng từ file ghi âm vào kho giọng").clicked() {
            self.mo = true;
        }
        self.cua_so(ui.ctx());
    }

    fn xu_ly(&self) {
        let (p, a, b) = (PathBuf::from(self.nguon.trim().trim_matches('"')), self.tu, self.den);
        let tt = self.tt.clone();
        {
            let mut t = tt.lock().unwrap();
            if t.ban {
                return;
            }
            *t = Tt { ban: true, tb: "Đang đọc và xử lý file...".into(), ..Default::default() };
        }
        std::thread::spawn(move || {
            let kq = cg::doc_nguon(&p).and_then(|(x, ts)| cg::chuan_bi(&x, ts, a, b));
            let mut t = tt.lock().unwrap();
            t.ban = false;
            match kq {
                Ok(m) => {
                    let (s, r) = cg::thong_ke(&m);
                    t.tb = format!("Đã xử lý: giọng mẫu {s:.1} giây (độ to {r:.3}). Bấm Nghe mẫu rồi Lưu vào kho.");
                    t.mau = Some(m);
                }
                Err(e) => (t.tb, t.loi) = (format!("Lỗi: {e}"), true),
            }
        });
    }

    fn cua_so(&mut self, ctx: &egui::Context) {
        if !self.mo {
            return;
        }
        let mut mo = self.mo;
        egui::Window::new("Thêm giọng mẫu").open(&mut mo).collapsible(false).default_width(560.0).show(ctx, |ui| {
            let (ban, tb, loi, co_mau, tk) = {
                let t = self.tt.lock().unwrap();
                (t.ban, t.tb.clone(), t.loi, t.mau.is_some(), t.mau.as_ref().map(|m| cg::thong_ke(m)))
            };
            if ban {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
            ui.label(RichText::new("Chọn đoạn nói rõ 6–15 giây, một người nói, không nhạc nền. File dài thì nhập giây bắt đầu – kết thúc.").small().weak());
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.nguon).desired_width(360.0).hint_text("đường dẫn file wav / mp3 / m4a / mp4..."));
                if ui.button("Chọn file...").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter("Âm thanh / video", &["wav", "mp3", "m4a", "ogg", "flac", "aac", "wma", "mp4", "mkv", "webm"])
                        .pick_file()
                    {
                        if self.ten.is_empty() {
                            self.ten = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        }
                        self.nguon = p.to_string_lossy().into_owned();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Từ giây");
                ui.add(egui::DragValue::new(&mut self.tu).range(0.0..=36000.0).speed(0.1).fixed_decimals(1));
                ui.label("đến giây");
                ui.add(egui::DragValue::new(&mut self.den).range(0.0..=36000.0).speed(0.1).fixed_decimals(1));
                ui.label(RichText::new("(đến = 0: hết file)").small().weak());
                if ui.add_enabled(!ban && !self.nguon.trim().is_empty(), egui::Button::new("Xử lý")).clicked() {
                    self.xu_ly();
                }
                if ui.add_enabled(co_mau, egui::Button::new("▶ Nghe mẫu")).clicked() {
                    if let Some(m) = self.tt.lock().unwrap().mau.clone() {
                        let p = crate::duong_dan::du_lieu().join("tam").join("them_giong_nghe.wav");
                        let _ = std::fs::create_dir_all(p.parent().unwrap());
                        if crate::viet_hoa::wwise::ghi_wav16(&p, &m, cg::TAN_SO).is_ok() {
                            giong_doc::phat_wav(&p);
                        }
                    }
                }
            });
            if let Some((s, _)) = tk {
                if !(5.0..=16.0).contains(&s) {
                    ui.label(RichText::new(format!("Giọng mẫu dài {s:.1} giây – tốt nhất 6–15 giây.")).color(Color32::YELLOW));
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Tên giọng:");
                ui.add(egui::TextEdit::singleline(&mut self.ten).desired_width(220.0).hint_text("vd nam_tre_2"));
                ui.checkbox(&mut self.ghi_de, "ghi đè nếu trùng tên");
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Gán luôn cho nhân vật (tùy chọn):");
                egui::ComboBox::from_id_salt("tg_phan").selected_text(PHAN[self.phan].1).show_ui(ui, |ui| {
                    for (i, (_, t)) in PHAN.iter().enumerate() {
                        ui.selectable_value(&mut self.phan, i, *t);
                    }
                });
                if self.phan > 0 {
                    ui.add(egui::TextEdit::singleline(&mut self.nguoi).desired_width(180.0).hint_text("vd unique_kliff"));
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(co_mau && !self.ten.trim().is_empty(), egui::Button::new(RichText::new("Lưu vào kho giọng").strong())).clicked() {
                    let m = self.tt.lock().unwrap().mau.clone().unwrap_or_default();
                    let mut tb = match cg::luu(&self.ten, &m, self.ghi_de) {
                        Ok(p) => {
                            SO_LAN_LUU.fetch_add(1, Ordering::SeqCst);
                            let file = p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
                            let mut s = format!("Đã lưu {file} vào kho giọng.");
                            if self.phan > 0 && !self.nguoi.trim().is_empty() {
                                match cg::gan(PHAN[self.phan].0, &self.nguoi, &file) {
                                    Ok(()) => s += &format!(" Đã gán cho {}.", self.nguoi.trim()),
                                    Err(e) => s += &format!(" Lỗi gán: {e}"),
                                }
                            }
                            s
                        }
                        Err(e) => format!("Lỗi: {e}"),
                    };
                    if tb.starts_with("Lỗi") {
                        tb = tb.to_string();
                    }
                    let mut t = self.tt.lock().unwrap();
                    t.loi = tb.starts_with("Lỗi");
                    t.tb = tb;
                }
            });
            if !tb.is_empty() {
                ui.label(if loi { RichText::new(tb).color(Color32::LIGHT_RED) } else { RichText::new(tb) });
            }
            ui.label(RichText::new(format!("Kho: {}", cg::kho_giong().display())).small().weak());
        });
        self.mo = mo;
    }
}
