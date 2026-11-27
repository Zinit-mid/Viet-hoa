//! Trang Giọng mẫu (menu chính): thư viện giọng dùng cho lồng tiếng game, thuyết minh phim, clone.
//! Thêm NHIỀU file / cả thư mục cùng lúc; bộ 3 mẫu cảm xúc (…__1_binh_thuong / __2_gap_rut / __3_buon) tự gom
//! thành một giọng; giọng dựng sẵn của VieNeu thêm được vào kho (chọn mẫu nào làm giọng chính).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::giong_doc;
use crate::thuyet_minh as tm;
use crate::viet_hoa::clone_giong as cg;

#[derive(Default)]
struct Tt {
    dang_ban: bool,
    thong_bao: String,
    loi: bool,
    tien: f32,
}

pub struct TrangGiongMau {
    kho: Vec<(String, f64)>,
    kho_so: usize,
    can_nap: bool,
    tim: String,
    chinh: usize,
    ghi_de: bool,
    xoa_cho: Option<String>,
    tt: Arc<Mutex<Tt>>,
    dung: Arc<AtomicBool>,
    them: crate::them_giong::ThemGiong,
}

impl Default for TrangGiongMau {
    fn default() -> Self {
        Self {
            kho: Vec::new(),
            kho_so: usize::MAX,
            can_nap: true,
            tim: String::new(),
            chinh: 2,
            ghi_de: false,
            xoa_cho: None,
            tt: Arc::new(Mutex::new(Tt::default())),
            dung: Arc::new(AtomicBool::new(false)),
            them: Default::default(),
        }
    }
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    crate::kieu::khoi(ui, tieu_de, noi);
}

impl TrangGiongMau {
    pub fn dang_ky_tt(&self) {
        let t = self.tt.clone();
        crate::tien_trinh::dang_ky(
            "giong_mau",
            crate::tien_trinh::co_dinh(self.dung.clone()),
            move || {
                let g = t.lock().unwrap();
                crate::tien_trinh::Anh {
                    dang_ban: g.dang_ban,
                    ten: "Giọng mẫu".into(),
                    thong_bao: g.thong_bao.clone(),
                    tien: (g.tien > 0.0).then_some(g.tien),
                    loi: g.loi,
                }
            },
        );
    }

    fn nen(
        &self,
        ten: &str,
        viec: impl FnOnce(&Arc<Mutex<Tt>>, &Arc<AtomicBool>) -> anyhow::Result<String> + Send + 'static,
    ) {
        {
            let mut t = self.tt.lock().unwrap();
            if t.dang_ban {
                return;
            }
            *t = Tt {
                dang_ban: true,
                thong_bao: format!("{ten}..."),
                ..Default::default()
            };
        }
        self.dung.store(false, Ordering::SeqCst);
        let (tt, dung) = (self.tt.clone(), self.dung.clone());
        std::thread::spawn(move || {
            let kq = viec(&tt, &dung);
            let mut t = tt.lock().unwrap();
            t.dang_ban = false;
            match kq {
                Ok(s) => t.thong_bao = s,
                Err(e) => (t.thong_bao, t.loi) = (format!("Không làm được: {e}"), true),
            }
        });
    }

    fn nhap(&self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        let (chinh, ghi_de) = (self.chinh, self.ghi_de);
        self.nen("Đang thêm giọng", move |tt, _| {
            let t2 = tt.clone();
            let kq = cg::nhap_nhieu(&files, chinh, ghi_de, &move |s, a, b| {
                let mut t = t2.lock().unwrap();
                t.thong_bao = s.to_string();
                t.tien = if b > 0 { a as f32 / b as f32 } else { 0.0 };
            });
            crate::them_giong::SO_LAN_LUU.fetch_add(1, Ordering::SeqCst);
            let mut s = format!("Đã thêm {} giọng vào kho", kq.them.len());
            if !kq.bo_qua.is_empty() {
                s += &format!(
                    " • bỏ qua {} giọng đã có tên (tick \"ghi đè\" nếu muốn thay)",
                    kq.bo_qua.len()
                );
            }
            if !kq.loi.is_empty() {
                s += &format!(
                    " • {} file lỗi, vd {}: {}",
                    kq.loi.len(),
                    kq.loi[0].0,
                    kq.loi[0].1
                );
            }
            Ok(s)
        });
    }

    /// Thêm giọng dựng sẵn của VieNeu vào kho (3 mẫu bình thường / gấp rút / buồn, mẫu `chinh` làm giọng chính).
    fn them_dung_san(&self, giong: &str) {
        let (g, chinh, ghi_de) = (giong.to_string(), self.chinh, self.ghi_de);
        self.nen(
            &format!("Đang thêm giọng {giong} vào kho"),
            move |tt, dung| {
                if !tm::co_bo(&g) {
                    let t2 = tt.clone();
                    if !tm::tao_bo(&[&g], dung, &move |s| {
                        t2.lock().unwrap().thong_bao = s.to_string()
                    })? {
                        return Ok("Đã dừng.".into());
                    }
                }
                let tam = crate::duong_dan::du_lieu()
                    .join("tam")
                    .join("giong_dung_san");
                let _ = std::fs::remove_dir_all(&tam);
                std::fs::create_dir_all(&tam)?;
                let thu = tm::thu_muc_bo().join(g.replace(' ', "_"));
                let ten = g.replace(' ', "_");
                let mut ds = Vec::new();
                for (n, k) in [(1, "binh_thuong"), (2, "gap_rut"), (3, "buon")] {
                    let dich = tam.join(format!("{ten}__{n}_{k}.wav"));
                    std::fs::copy(thu.join(format!("{k}.wav")), &dich)?;
                    ds.push(dich);
                }
                let kq = cg::nhap_nhieu(&ds, chinh, ghi_de, &|_, _, _| {});
                let _ = std::fs::remove_dir_all(&tam);
                crate::them_giong::SO_LAN_LUU.fetch_add(1, Ordering::SeqCst);
                Ok(if kq.them.is_empty() && !kq.bo_qua.is_empty() {
                    format!("Giọng {g} đã có trong kho (tick \"ghi đè\" nếu muốn thay).")
                } else {
                    format!(
                        "Đã thêm giọng {g} vào kho ({} file, giọng chính lấy từ mẫu số {chinh}).",
                        kq.them.len()
                    )
                })
            },
        );
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Giọng mẫu");
        ui.label("Thư viện giọng dùng cho lồng tiếng game, thuyết minh phim và clone. Thêm nhiều file cùng lúc, hoặc lấy giọng dựng sẵn của VieNeu về kho.");
        ui.add_space(6.0);
        let so = crate::them_giong::SO_LAN_LUU.load(Ordering::SeqCst);
        if so != self.kho_so || self.can_nap {
            self.kho_so = so;
            self.can_nap = false;
            self.kho = cg::liet_ke_chinh()
                .into_iter()
                .map(|f| {
                    let g = cg::giay_cua(&f);
                    (f, g)
                })
                .collect();
        }
        let (ban, tb, loi) = {
            let t = self.tt.lock().unwrap();
            (t.dang_ban, t.thong_bao.clone(), t.loi)
        };

        the(ui, "THÊM GIỌNG", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !ban,
                        egui::Button::new(RichText::new("+ Thêm nhiều file...").strong()),
                    )
                    .clicked()
                {
                    if let Some(f) = rfd::FileDialog::new()
                        .add_filter(
                            "Âm thanh",
                            &["wav", "mp3", "m4a", "ogg", "flac", "aac", "opus"],
                        )
                        .pick_files()
                    {
                        self.nhap(f);
                    }
                }
                if ui
                    .add_enabled(!ban, egui::Button::new("+ Thêm cả thư mục..."))
                    .clicked()
                {
                    if let Some(d) = rfd::FileDialog::new().pick_folder() {
                        self.nhap(cg::tim_am_thanh(&d));
                    }
                }
                self.them.nut(ui);
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Bộ 3 mẫu (…__1_binh_thuong / __2_gap_rut / __3_buon): giọng chính lấy từ mẫu số");
                for (n, t) in [(1, "1 bình thường"), (2, "2 gấp rút"), (3, "3 buồn")] {
                    ui.selectable_value(&mut self.chinh, n, t);
                }
                crate::kieu::cong_tac(ui, &mut self.ghi_de, "ghi đè nếu trùng tên");
            });
            ui.label(RichText::new("File riêng lẻ: mỗi file thành một giọng, tên theo tên file (tự cắt lặng, mono 24 kHz, chuẩn hóa độ to). Tên dạng Giọng__1_binh_thuong, Giọng__2_gap_rut, Giọng__3_buon được gom thành MỘT giọng có cảm xúc (dùng cho thuyết minh).").small().weak());
        });

        the(
            ui,
            &format!("GIỌNG ĐÃ CÓ TRONG KHO ({})", self.kho.len()),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Tìm:");
                    ui.add(egui::TextEdit::singleline(&mut self.tim).desired_width(220.0));
                    if ui.button("Làm mới").clicked() {
                        self.can_nap = true;
                    }
                });
                let q = self.tim.to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .auto_shrink(false)
                    .show(ui, |ui| {
                        for (f, giay) in self.kho.clone() {
                            if !q.is_empty() && !f.to_lowercase().contains(&q) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                if ui.small_button("▶").on_hover_text("Nghe").clicked() {
                                    giong_doc::phat_wav(&cg::kho_giong().join(&f));
                                }
                                ui.label(&f);
                                ui.label(RichText::new(format!("{giay:.1}s")).small().weak());
                                if cg::co_cam_xuc(&f) {
                                    ui.label(
                                        RichText::new("có cảm xúc")
                                            .small()
                                            .color(Color32::LIGHT_GREEN),
                                    );
                                }
                                if self.xoa_cho.as_deref() == Some(&f) {
                                    ui.label(
                                        RichText::new("Xóa giọng này?").color(Color32::YELLOW),
                                    );
                                    if ui.small_button("Xóa").clicked() {
                                        let r = cg::xoa(&f);
                                        self.tt.lock().unwrap().thong_bao = match r {
                                            Ok(()) => format!("Đã xóa {f}."),
                                            Err(e) => format!("Lỗi xóa: {e}"),
                                        };
                                        self.xoa_cho = None;
                                        self.can_nap = true;
                                        crate::them_giong::SO_LAN_LUU
                                            .fetch_add(1, Ordering::SeqCst);
                                    }
                                    if ui.small_button("Thôi").clicked() {
                                        self.xoa_cho = None;
                                    }
                                } else if ui.small_button("🗑").on_hover_text("Xóa").clicked() {
                                    self.xoa_cho = Some(f.clone());
                                }
                            });
                        }
                    });
            },
        );

        the(
            ui,
            "GIỌNG DỰNG SẴN CỦA VIENEU (kể chuyện / đọc truyện)",
            |ui| {
                ui.label(RichText::new("Bấm \"Thêm vào kho\" để lấy giọng về kho (kèm bộ 3 mẫu cảm xúc, giọng chính theo mẫu đã chọn ở trên). Lần đầu mỗi giọng tạo mẫu ~1 phút.").small().weak());
                for (g, mo_ta) in tm::GIONG.iter() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{g} ({mo_ta})"));
                        let thu = tm::thu_muc_bo().join(g.replace(' ', "_"));
                        for (n, k) in [(1, "binh_thuong"), (2, "gap_rut"), (3, "buon")] {
                            let p = thu.join(format!("{k}.wav"));
                            if ui
                                .add_enabled(
                                    p.is_file(),
                                    egui::Button::new(format!("▶{n}")).small(),
                                )
                                .on_hover_text(k)
                                .clicked()
                            {
                                giong_doc::phat_wav(&p);
                            }
                        }
                        if ui
                            .add_enabled(!ban, egui::Button::new("Thêm vào kho"))
                            .clicked()
                        {
                            self.them_dung_san(g);
                        }
                    });
                }
            },
        );

        if !tb.is_empty() {
            ui.label(RichText::new(tb).color(if loi {
                Color32::LIGHT_RED
            } else {
                Color32::LIGHT_GREEN
            }));
        }
    }
}
