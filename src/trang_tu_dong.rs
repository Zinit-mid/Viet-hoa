//! Trang Làm tự động: chọn game → bấm một nút → dịch + lồng tiếng + cài. Phim: xem mục Phụ đề phim (cũng một nút).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::tu_dong::{self, Bc, TuyChon};
use crate::viet_hoa::tim_game::{self, Game, LoaiGame};
use crate::{dich, duong_dan};

#[derive(Default)]
struct Tt {
    dang_ban: bool,
    thong_bao: String,
    loi: bool,
    xong: bool,
    tien: f32,
}

pub struct TrangTuDong {
    games: Option<Vec<Game>>,
    dang_tim: Arc<Mutex<Option<Vec<Game>>>>,
    chon: Option<usize>,
    t: TuyChon,
    tt: Arc<Mutex<Tt>>,
    dung: Arc<AtomicBool>,
    da_goi: bool,
}

impl Default for TrangTuDong {
    fn default() -> Self {
        Self {
            games: None,
            dang_tim: Arc::new(Mutex::new(None)),
            chon: None,
            t: TuyChon { claude: false, gioi_han: 1.0, dich: true, long_tieng: true, cai: true, khong_dau: false },
            tt: Arc::new(Mutex::new(Tt::default())),
            dung: Arc::new(AtomicBool::new(false)),
            da_goi: false,
        }
    }
}

fn mo_ta(l: LoaiGame) -> &'static str {
    match l {
        LoaiGame::CrimsonDesert => "Dịch 3 đợt chữ → xuất gói cho DMM → lồng tiếng nhân vật (A) và dân thường (B) → 3 file .zip ra Desktop. Lồng tiếng rất lâu (hàng giờ), cần VieNeu GPU + Wwise.",
        LoaiGame::Rise => "Dịch chữ + phụ đề + lời thoại → lồng tiếng → cài gói mod vào game (không sửa file gốc).",
        LoaiGame::Tr2013 => "Dịch chữ + phụ đề → lồng tiếng → cài vào patch2 (giữ bản gốc).",
        LoaiGame::Shadow => "Dịch chữ → cài gói mod vào game. (Chưa có lồng tiếng.)",
        LoaiGame::Khac => "Tự dò file chữ → dịch → ghi vào game (có sao lưu bản gốc). Chưa có lồng tiếng cho game này.",
    }
}

impl TrangTuDong {
    fn chay(&mut self, g: Game) {
        {
            let mut t = self.tt.lock().unwrap();
            if t.dang_ban {
                return;
            }
            *t = Tt { dang_ban: true, thong_bao: "Đang bắt đầu...".into(), ..Default::default() };
        }
        let (tt, dung, tuy) = (self.tt.clone(), self.dung.clone(), self.t.clone());
        std::thread::spawn(move || {
            let (a, b) = (tt.clone(), tt.clone());
            let bao = move |s: &str| a.lock().unwrap().thong_bao = s.to_string();
            let tien = move |x: usize, y: usize, _| b.lock().unwrap().tien = if y > 0 { x as f32 / y as f32 } else { 0.0 };
            let kq = tu_dong::chay(&g, &tuy, &dung, &Bc { bao: &bao, tien: &tien });
            let mut t = tt.lock().unwrap();
            t.dang_ban = false;
            match kq {
                Ok(s) => (t.thong_bao, t.xong) = (s, true),
                Err(e) => (t.thong_bao, t.loi) = (format!("Không làm được: {e}"), true),
            }
            duong_dan::nhat_ky(&format!("Làm tự động {}: {}", g.ten, t.thong_bao));
        });
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Làm tự động");
        ui.label("Chọn game > bấm Bắt đầu. App tự dịch, lồng tiếng và cài; dừng lúc nào cũng được, bấm lại là làm tiếp. Phim: dùng mục Phụ đề phim (cũng chỉ một nút).");
        ui.add_space(8.0);
        if self.games.is_none() {
            if let Some(g) = self.dang_tim.lock().unwrap().take() {
                self.games = Some(g);
            } else {
                if !self.da_goi {
                    self.da_goi = true;
                    let o = self.dang_tim.clone();
                    std::thread::spawn(move || *o.lock().unwrap() = Some(tim_game::tim_tat_ca()));
                }
                ui.label("Đang tìm game trên máy...");
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
        }
        let (ban, tb, loi, xong, tien) = {
            let t = self.tt.lock().unwrap();
            (t.dang_ban, t.thong_bao.clone(), t.loi, t.xong, t.tien)
        };
        if ban {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
        }
        let games = self.games.clone().unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            ui.label("Game:");
            let ten = self.chon.and_then(|i| games.get(i)).map(|g| g.ten.clone()).unwrap_or_else(|| "chọn game...".into());
            ui.add_enabled_ui(!ban, |ui| {
                egui::ComboBox::from_id_salt("td_game").selected_text(ten).width(420.0).show_ui(ui, |ui| {
                    for (i, g) in games.iter().enumerate() {
                        ui.selectable_value(&mut self.chon, Some(i), &g.ten);
                    }
                });
            });
            if ui.add_enabled(!ban, egui::Button::new("Tìm lại")).clicked() {
                self.games = None;
                self.da_goi = false;
            }
        });
        let Some(g) = self.chon.and_then(|i| games.get(i)).cloned() else { return };
        let loai = g.loai();
        ui.label(RichText::new(mo_ta(loai)).small().weak());
        ui.add_space(6.0);
        ui.add_enabled_ui(!ban, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.t.dich, "Dịch chữ");
                if matches!(loai, LoaiGame::CrimsonDesert | LoaiGame::Rise | LoaiGame::Tr2013) {
                    ui.checkbox(&mut self.t.long_tieng, "Lồng tiếng");
                }
                if loai != LoaiGame::CrimsonDesert {
                    ui.checkbox(&mut self.t.cai, "Cài vào game");
                    ui.checkbox(&mut self.t.khong_dau, "không dấu (nếu font game thiếu dấu)");
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Máy dịch:");
                ui.selectable_value(&mut self.t.claude, false, "Offline (miễn phí)");
                ui.selectable_value(&mut self.t.claude, true, "Claude (chất lượng cao, tốn tiền)");
                if self.t.claude {
                    for v in [1.0, 2.0, 5.0] {
                        ui.selectable_value(&mut self.t.gioi_han, v, format!("{v:.0} USD"));
                    }
                    if !dich::claude::co_khoa() {
                        ui.label(RichText::new("chưa có mã Claude (Cài đặt) – sẽ dịch offline").color(Color32::YELLOW));
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!ban && (self.t.dich || self.t.long_tieng || self.t.cai), egui::Button::new(RichText::new("Bắt đầu").strong()))
                .clicked()
            {
                self.chay(g.clone());
            }
            if ui.add_enabled(ban, egui::Button::new("Dừng")).clicked() {
                self.dung.store(true, Ordering::SeqCst);
            }
        });
        if ban {
            ui.add(egui::ProgressBar::new(self.tt.lock().unwrap().tien.max(tien)).show_percentage().animate(tien == 0.0));
        }
        if !tb.is_empty() {
            ui.add_space(6.0);
            let m = if loi { Color32::LIGHT_RED } else if xong { Color32::LIGHT_GREEN } else { Color32::YELLOW };
            ui.label(RichText::new(tb).color(m));
        }
        ui.label(RichText::new("Tắt game trước khi cài. Sửa bản dịch cho đẹp: mục Sửa bản dịch (câu đã sửa được giữ khi chạy lại).").small().weak());
    }
}
