//! Trang Clone giọng: file ghi âm → giọng mẫu → nghe thử → lưu kho → gán cho người nói từng game.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::giong_doc;
use crate::viet_hoa::clone_giong as cg;

#[derive(Default)]
struct Chung {
    thong_bao: String,
    dang_ban: bool,
    /// giọng mẫu đã xử lý (mono 24 kHz)
    mau: Option<Vec<f32>>,
}

pub struct TrangCloneGiong {
    nguon: String,
    bat_dau: f64,
    ket_thuc: f64,
    cau_thu: String,
    ten_luu: String,
    ghi_de: bool,
    phan: usize,
    nguoi: String,
    chon: Option<String>,
    kho: Vec<String>,
    c: Arc<Mutex<Chung>>,
    dung: Arc<AtomicBool>,
}

const PHAN: [(&str, &str); 3] = [
    ("A", "Crimson Desert – nhân vật có tên (A)"),
    ("B", "Crimson Desert – dân thường (B)"),
    ("TR2013", "Tomb Raider (2013)"),
];

impl Default for TrangCloneGiong {
    fn default() -> Self {
        Self {
            nguon: String::new(),
            bat_dau: 0.0,
            ket_thuc: 0.0,
            cau_thu:
                "Xin chào, tôi là một nhân vật trong game. Hãy cẩn thận, phía trước có kẻ thù."
                    .into(),
            ten_luu: String::new(),
            ghi_de: false,
            phan: 0,
            nguoi: String::new(),
            chon: None,
            kho: cg::liet_ke(),
            c: Arc::new(Mutex::new(Chung::default())),
            dung: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    crate::kieu::khoi(ui, tieu_de, noi);
}

impl TrangCloneGiong {
    /// Đăng ký tiến trình toàn cục (thanh ở đáy cửa sổ).
    pub fn dang_ky_tt(&self) {
        let c = self.c.clone();
        crate::tien_trinh::dang_ky(
            "clone_giong",
            crate::tien_trinh::co_dinh(self.dung.clone()),
            move || {
                let g = c.lock().unwrap();
                crate::tien_trinh::Anh {
                    dang_ban: g.dang_ban,
                    ten: "Clone giọng".into(),
                    thong_bao: g.thong_bao.clone(),
                    tien: None,
                    loi: g.thong_bao.starts_with("Lỗi"),
                }
            },
        );
    }

    fn nen(&self, viec: impl FnOnce(&Arc<Mutex<Chung>>, &Arc<AtomicBool>) + Send + 'static) {
        {
            let mut c = self.c.lock().unwrap();
            if c.dang_ban {
                return;
            }
            c.dang_ban = true;
        }
        self.dung.store(false, Ordering::SeqCst);
        let (c, d) = (self.c.clone(), self.dung.clone());
        std::thread::spawn(move || {
            viec(&c, &d);
            c.lock().unwrap().dang_ban = false;
        });
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Clone giọng");
        ui.label("Chọn file ghi âm (6–15 giây tiếng rõ, không nhạc nền) > cắt, bỏ lặng, mono 24 kHz, lọc 70 Hz, chuẩn hóa độ to > nghe thử bằng VieNeu > lưu vào kho giọng > gán cho nhân vật từng game.");
        ui.add_space(8.0);
        let (ban, tb, co_mau, mau_tk) = {
            let c = self.c.lock().unwrap();
            (
                c.dang_ban,
                c.thong_bao.clone(),
                c.mau.is_some(),
                c.mau.as_ref().map(|m| cg::thong_ke(m)),
            )
        };
        if ban {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }

        the(ui, "1. FILE GHI ÂM", |ui| {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.nguon)
                        .desired_width(520.0)
                        .hint_text("đường dẫn file wav / mp3 / m4a / mp4..."),
                );
                if ui.button("Chọn file...").clicked() {
                    if let Some(p) = rfd::FileDialog::new()
                        .add_filter(
                            "Âm thanh / video",
                            &[
                                "wav", "mp3", "m4a", "ogg", "flac", "aac", "wma", "mp4", "mkv",
                                "webm",
                            ],
                        )
                        .pick_file()
                    {
                        self.nguon = p.to_string_lossy().into_owned();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Từ giây");
                ui.add(
                    egui::DragValue::new(&mut self.bat_dau)
                        .range(0.0..=36000.0)
                        .speed(0.1)
                        .fixed_decimals(1),
                );
                ui.label("đến giây");
                ui.add(
                    egui::DragValue::new(&mut self.ket_thuc)
                        .range(0.0..=36000.0)
                        .speed(0.1)
                        .fixed_decimals(1),
                );
                ui.label(RichText::new("(đến = 0: hết file)").small().weak());
            });
            if crate::kieu::nut_tien_do(
                ui,
                "clone_giong",
                "Xử lý giọng mẫu",
                "Đang xử lý",
                crate::kieu::CAM,
                !ban && !self.nguon.trim().is_empty(),
            ) {
                let (p, a, b) = (
                    PathBuf::from(self.nguon.trim().trim_matches('"')),
                    self.bat_dau,
                    self.ket_thuc,
                );
                self.nen(move |c, _| {
                    c.lock().unwrap().thong_bao = "Đang đọc và xử lý file...".into();
                    let kq = cg::doc_nguon(&p).and_then(|(x, ts)| cg::chuan_bi(&x, ts, a, b));
                    let mut g = c.lock().unwrap();
                    match kq {
                        Ok(m) => {
                            let (s, r) = cg::thong_ke(&m);
                            g.thong_bao = format!("Đã tạo giọng mẫu {s:.1} giây (độ to {r:.3}). Bấm Nghe mẫu để kiểm tra.");
                            g.mau = Some(m);
                        }
                        Err(e) => {
                            g.thong_bao = format!("Lỗi: {e}");
                            g.mau = None;
                        }
                    }
                });
            }
        });

        the(ui, "2. NGHE THỬ", |ui| {
            if let Some((s, _)) = mau_tk {
                if !(5.0..=16.0).contains(&s) {
                    ui.label(
                        RichText::new(format!("Giọng mẫu dài {s:.1} giây – tốt nhất 6–15 giây."))
                            .color(Color32::YELLOW),
                    );
                }
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(co_mau, egui::Button::new("▶ Nghe mẫu đã xử lý"))
                    .clicked()
                {
                    let m = self.c.lock().unwrap().mau.clone();
                    if let Some(m) = m {
                        let p = crate::duong_dan::du_lieu()
                            .join("tam")
                            .join("clone_nghe_mau.wav");
                        let _ = std::fs::create_dir_all(p.parent().unwrap());
                        if crate::viet_hoa::wwise::ghi_wav16(&p, &m, cg::TAN_SO).is_ok() {
                            giong_doc::phat_wav(&p);
                        }
                    }
                }
            });
            ui.add(
                egui::TextEdit::multiline(&mut self.cau_thu)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
            if ui
                .add_enabled(
                    !ban && co_mau && crate::viet_hoa::giong_vieneu::tim_vieneu().is_some(),
                    egui::Button::new("▶ VieNeu đọc câu này bằng giọng mẫu"),
                )
                .clicked()
            {
                let (m, chu) = (
                    self.c.lock().unwrap().mau.clone().unwrap_or_default(),
                    self.cau_thu.clone(),
                );
                self.nen(move |c, d| {
                    c.lock().unwrap().thong_bao =
                        "VieNeu đang nạp mô hình và đọc (~15–30 giây)...".into();
                    let kq = cg::nghe_thu(&m, &chu, d, &|s| {
                        c.lock().unwrap().thong_bao = s.to_string()
                    });
                    let mut g = c.lock().unwrap();
                    match kq {
                        Ok(Some(p)) => {
                            g.thong_bao = "Đang phát giọng đọc thử.".into();
                            giong_doc::phat_wav(&p);
                        }
                        Ok(None) => g.thong_bao = "Đã dừng.".into(),
                        Err(e) => g.thong_bao = format!("Lỗi: {e}"),
                    }
                });
            }
            if crate::viet_hoa::giong_vieneu::tim_vieneu().is_none() {
                ui.label(
                    RichText::new("Không thấy app VieNeu (bản GPU) – cần để nghe thử.")
                        .color(Color32::YELLOW),
                );
            }
        });

        the(ui, "3. LƯU VÀO KHO GIỌNG", |ui| {
            ui.horizontal(|ui| {
                ui.label("Tên giọng:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.ten_luu)
                        .desired_width(220.0)
                        .hint_text("vd nam_tre_2"),
                );
                crate::kieu::cong_tac(ui, &mut self.ghi_de, "ghi đè nếu trùng tên");
                if ui
                    .add_enabled(
                        co_mau && !self.ten_luu.trim().is_empty(),
                        egui::Button::new("Lưu"),
                    )
                    .clicked()
                {
                    let m = self.c.lock().unwrap().mau.clone().unwrap_or_default();
                    let tb = match cg::luu(&self.ten_luu, &m, self.ghi_de) {
                        Ok(p) => format!("Đã lưu {}", p.display()),
                        Err(e) => format!("Lỗi: {e}"),
                    };
                    self.c.lock().unwrap().thong_bao = tb;
                    self.kho = cg::liet_ke();
                }
            });
            ui.label(
                RichText::new(format!("Kho: {}", cg::kho_giong().display()))
                    .small()
                    .weak(),
            );
        });

        the(ui, "4. GÁN GIỌNG CHO NHÂN VẬT", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Game / phần:");
                egui::ComboBox::from_id_salt("clone_phan")
                    .selected_text(PHAN[self.phan].1)
                    .show_ui(ui, |ui| {
                        for (i, (_, t)) in PHAN.iter().enumerate() {
                            ui.selectable_value(&mut self.phan, i, *t);
                        }
                    });
                ui.label("Người nói:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.nguoi)
                        .desired_width(200.0)
                        .hint_text("vd unique_kliff"),
                );
                egui::ComboBox::from_id_salt("clone_nguoi")
                    .selected_text("chọn có sẵn")
                    .show_ui(ui, |ui| {
                        for n in cg::nguoi_da_co(PHAN[self.phan].0) {
                            if ui.selectable_label(false, &n).clicked() {
                                self.nguoi = n;
                            }
                        }
                    });
            });
            ui.label(
                RichText::new("Giọng trong kho (bấm chọn, ▶ để nghe):")
                    .small()
                    .weak(),
            );
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    for n in self.kho.clone() {
                        ui.horizontal(|ui| {
                            if ui.small_button("▶").clicked() {
                                giong_doc::phat_wav(&cg::kho_giong().join(&n));
                            }
                            if ui
                                .selectable_label(self.chon.as_deref() == Some(&n), &n)
                                .clicked()
                            {
                                self.chon = Some(n.clone());
                            }
                        });
                    }
                });
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        self.chon.is_some() && !self.nguoi.trim().is_empty(),
                        egui::Button::new(RichText::new("Gán giọng đã chọn").strong()),
                    )
                    .clicked()
                {
                    let tb = match cg::gan(
                        PHAN[self.phan].0,
                        &self.nguoi,
                        self.chon.as_deref().unwrap_or(""),
                    ) {
                        Ok(()) => format!(
                            "Đã gán {} cho {} – lần lồng tiếng sau sẽ dùng giọng này.",
                            self.chon.clone().unwrap_or_default(),
                            self.nguoi.trim()
                        ),
                        Err(e) => format!("Lỗi: {e}"),
                    };
                    self.c.lock().unwrap().thong_bao = tb;
                }
                if ui.button("Làm mới kho").clicked() {
                    self.kho = cg::liet_ke();
                }
            });
            ui.label(RichText::new("Gán xong, chạy lại Lồng tiếng của game: giọng mới thay giọng cũ của người đó (câu đã lồng bằng giọng cũ được làm lại). Bản cũ của phan_bo.json được sao lưu 1 lần (.truoc_gan_tay).").small().weak());
        });

        if !tb.is_empty() {
            ui.label(if tb.starts_with("Lỗi") {
                RichText::new(tb).color(Color32::LIGHT_RED)
            } else {
                RichText::new(tb)
            });
        }
    }
}
