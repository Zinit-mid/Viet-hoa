//! Trang "Việt hóa game": thư viện game + Dò file chữ (giống app cũ, gọn hơn).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::viet_hoa::do_file::{self, ViecFile};
use crate::viet_hoa::tim_game::{self, Game, LoaiGame};
use crate::viet_hoa::viec_dich::BaoCao;
use crate::{dich, duong_dan};

/// Tình hình việc nền (dò, dịch, ghi...).
#[derive(Default)]
struct TienTrinh {
    viec: Option<String>,
    thong_bao: String,
    xong: usize,
    tong: usize,
    usd: f64,
}

struct BaoTt(Arc<Mutex<TienTrinh>>);

impl BaoCao for BaoTt {
    fn bao(&self, s: &str) {
        self.0.lock().unwrap().thong_bao = s.to_string();
    }
    fn tien_do(&self, xong: usize, tong: usize, usd: f64) {
        let mut t = self.0.lock().unwrap();
        t.xong = xong;
        t.tong = tong;
        t.usd = usd;
    }
}

pub struct TrangVietHoa {
    games: Option<Vec<Game>>,
    dang_tim: Arc<Mutex<Option<Vec<Game>>>>,
    chon: Option<usize>,
    viec: Option<Arc<Mutex<ViecFile>>>,
    tt: Arc<Mutex<TienTrinh>>,
    dung: Arc<AtomicBool>,
    gioi_han: f64,
    khong_dau: bool,
    hoi_tra_lai: bool,
}

impl Default for TrangVietHoa {
    fn default() -> Self {
        Self {
            games: None,
            dang_tim: Arc::new(Mutex::new(None)),
            chon: None,
            viec: None,
            tt: Arc::new(Mutex::new(TienTrinh::default())),
            dung: Arc::new(AtomicBool::new(false)),
            gioi_han: 1.0,
            khong_dau: false,
            hoi_tra_lai: false,
        }
    }
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style())
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(tieu_de)
                    .strong()
                    .color(Color32::from_rgb(64, 160, 255)),
            );
            ui.add_space(4.0);
            noi(ui);
        });
    ui.add_space(8.0);
}

impl TrangVietHoa {
    fn tim_lai(&mut self) {
        self.games = None;
        let o = self.dang_tim.clone();
        std::thread::spawn(move || {
            *o.lock().unwrap() = Some(tim_game::tim_tat_ca());
        });
    }

    fn dang_ban(&self) -> bool {
        self.tt.lock().unwrap().viec.is_some()
    }

    /// Chạy việc nền với ViecFile; tự cập nhật tiến trình.
    fn chay(
        &self,
        ten: &str,
        viec: impl FnOnce(&mut ViecFile, &BaoTt) -> Result<String, String> + Send + 'static,
    ) {
        let Some(v) = self.viec.clone() else { return };
        {
            let mut t = self.tt.lock().unwrap();
            if t.viec.is_some() {
                return;
            }
            *t = TienTrinh {
                viec: Some(ten.to_string()),
                thong_bao: format!("{ten}..."),
                ..Default::default()
            };
        }
        let tt = self.tt.clone();
        std::thread::spawn(move || {
            let bc = BaoTt(tt.clone());
            let kq = {
                let mut g = v.lock().unwrap();
                viec(&mut g, &bc)
            };
            let mut t = tt.lock().unwrap();
            t.viec = None;
            t.thong_bao = match kq {
                Ok(s) => s,
                Err(e) => format!("Lỗi: {e}"),
            };
            duong_dan::nhat_ky(&format!("Việt hóa game: {}", t.thong_bao));
        });
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Việt hóa game");
        ui.label("Chọn game > Dò file chữ > chọn file > dịch (offline / Claude) hoặc tự dịch bằng Excel > Ghi vào game.");
        ui.add_space(8.0);
        if self.games.is_none() {
            if let Some(g) = self.dang_tim.lock().unwrap().take() {
                self.games = Some(g);
            } else {
                static DA_GOI: AtomicBool = AtomicBool::new(false);
                if !DA_GOI.swap(true, Ordering::SeqCst) {
                    let o = self.dang_tim.clone();
                    std::thread::spawn(move || *o.lock().unwrap() = Some(tim_game::tim_tat_ca()));
                }
                ui.label("Đang tìm game trên máy...");
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
        }

        // ---- thư viện game ----
        let mut mo: Option<usize> = None;
        let mut tim_lai = false;
        let mut bo_tay: Option<PathBuf> = None;
        // Đã chọn game → thu gọn thư viện thành 1 dòng cho gọn
        if let Some(i) = self.chon {
            let mut doi = false;
            the(ui, "GAME ĐANG CHỌN", |ui| {
                ui.horizontal(|ui| {
                    let g = &self.games.as_ref().unwrap()[i];
                    ui.label(RichText::new(&g.ten).strong().size(18.0));
                    ui.label(
                        RichText::new(g.thu_muc.display().to_string())
                            .small()
                            .weak(),
                    );
                    if ui
                        .add_enabled(!self.dang_ban(), egui::Button::new("Đổi game"))
                        .clicked()
                    {
                        doi = true;
                    }
                });
            });
            if doi {
                self.chon = None;
                self.viec = None;
                *self.tt.lock().unwrap() = TienTrinh::default();
                return;
            }
        } else {
            the(ui, "THƯ VIỆN GAME", |ui| {
                let games = self.games.as_ref().unwrap();
                egui::Grid::new("game")
                    .striped(true)
                    .num_columns(4)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        for (i, g) in games.iter().enumerate() {
                            let nguon = match g.nguon.as_str() {
                                "steam" => "Steam",
                                "epic" => "Epic",
                                "gog" => "GOG",
                                _ => "Tự thêm",
                            };
                            ui.label(RichText::new(&g.ten).strong());
                            ui.label(RichText::new(nguon).weak());
                            let rieng = match g.loai() {
                                LoaiGame::CrimsonDesert => {
                                    Some("Crimson Desert: công cụ riêng ở giai đoạn 4")
                                }
                                LoaiGame::Rise | LoaiGame::Tr2013 | LoaiGame::Shadow => {
                                    Some("Tomb Raider: công cụ riêng ở giai đoạn 5")
                                }
                                LoaiGame::Khac => None,
                            };
                            ui.horizontal(|ui| {
                                let chon = self.chon == Some(i);
                                if ui
                                    .add(egui::Button::new("Dò file chữ").selected(chon))
                                    .clicked()
                                {
                                    mo = Some(i);
                                }
                                if g.nguon == "tay" && ui.small_button("Bỏ").clicked() {
                                    bo_tay = Some(g.thu_muc.clone());
                                }
                            });
                            match rieng {
                                Some(r) => {
                                    ui.label(RichText::new(r).small().color(Color32::YELLOW))
                                }
                                None => ui.label(
                                    RichText::new(g.thu_muc.display().to_string())
                                        .small()
                                        .weak(),
                                ),
                            };
                            ui.end_row();
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.button("+ Thêm game (chọn thư mục)").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .set_title("Chọn thư mục game")
                            .pick_folder()
                        {
                            tim_game::them_game_tay(&p, None);
                            tim_lai = true;
                        }
                    }
                    if ui.button("Tìm lại").clicked() {
                        tim_lai = true;
                    }
                });
            });
        }
        if let Some(p) = bo_tay {
            tim_game::bo_game_tay(&p);
            tim_lai = true;
        }
        if tim_lai {
            self.tim_lai();
            self.chon = None;
            self.viec = None;
            return;
        }
        if let Some(i) = mo {
            if !self.dang_ban() {
                let g = &self.games.as_ref().unwrap()[i];
                self.chon = Some(i);
                let mut v = ViecFile::moi(&g.thu_muc, &g.ten);
                v.doc_chon();
                self.dung = v.viec.dung.clone();
                self.viec = Some(Arc::new(Mutex::new(v)));
                *self.tt.lock().unwrap() = TienTrinh::default();
            }
        }

        let (Some(i), Some(viec)) = (self.chon, self.viec.clone()) else {
            return;
        };
        let ten_game = self.games.as_ref().unwrap()[i].ten.clone();
        let ban = self.dang_ban();
        if ban {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }

        // ---- tiến trình ----
        {
            let t = self.tt.lock().unwrap();
            if t.viec.is_some() || !t.thong_bao.is_empty() {
                the(ui, "TIẾN TRÌNH", |ui| {
                    if t.viec.is_some() {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(&t.thong_bao);
                        });
                        if t.tong > 0 {
                            ui.add(
                                egui::ProgressBar::new(t.xong as f32 / t.tong as f32)
                                    .text(format!("{}/{} câu • {:.3} USD", t.xong, t.tong, t.usd)),
                            );
                        }
                        if ui.button("■ Dừng").clicked() {
                            self.dung.store(true, Ordering::SeqCst);
                        }
                    } else {
                        ui.label(&t.thong_bao);
                    }
                });
            }
        }

        let Ok(mut v) = viec.try_lock() else {
            ui.label(RichText::new(format!("{ten_game}: đang làm việc nền...")).weak());
            return;
        };

        // ---- dò file ----
        let mut can_doc_lai = false;
        let mut quet = false;
        the(ui, &format!("DÒ FILE CHỮ – {ten_game}"), |ui| {
            match &v.kq_quet {
                None => {
                    ui.label("Chưa dò game này.");
                }
                Some(kq) => {
                    let engine = if kq.engine.is_empty() {
                        "không rõ".to_string()
                    } else {
                        kq.engine.clone()
                    };
                    ui.label(format!(
                        "Dò lúc {} • engine: {engine} • {} file chữ tìm thấy (đã xem {} file)",
                        kq.luc,
                        kq.file.len(),
                        kq.so_tep
                    ));
                    for g in &kq.goi {
                        ui.label(
                            RichText::new(format!(
                                "Chưa hỗ trợ: {} – {} file, {} MB",
                                g.loai,
                                g.so,
                                g.co >> 20
                            ))
                            .small()
                            .weak(),
                        );
                    }
                }
            }
            if ui
                .add_enabled(
                    !ban,
                    egui::Button::new(if v.kq_quet.is_some() {
                        "Dò lại"
                    } else {
                        "▶ Dò file chữ"
                    }),
                )
                .clicked()
            {
                quet = true;
            }
            let Some(kq) = v.kq_quet.clone() else { return };
            if kq.file.is_empty() {
                ui.label(RichText::new("Không thấy file chữ đọc được. Game dùng gói nén riêng – cần công cụ riêng cho game này.").color(Color32::YELLOW));
                return;
            }
            ui.add_space(4.0);
            ui.label(
                RichText::new("Chọn các file cần Việt hóa (đã chọn sẵn file tốt nhất):").small(),
            );
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .id_salt("ds_file")
                .show(ui, |ui| {
                    egui::Grid::new("file")
                        .striped(true)
                        .num_columns(5)
                        .spacing([10.0, 4.0])
                        .show(ui, |ui| {
                            for f in kq.file.iter().take(60) {
                                let mut co = v.chon.contains(&f.tuong_doi);
                                if ui
                                    .add_enabled(!ban, egui::Checkbox::without_text(&mut co))
                                    .changed()
                                {
                                    if co {
                                        v.chon.push(f.tuong_doi.clone());
                                    } else {
                                        v.chon.retain(|x| x != &f.tuong_doi);
                                    }
                                    can_doc_lai = true;
                                }
                                ui.label(&f.tuong_doi).on_hover_text(f.mau.join("\n\n"));
                                ui.label(RichText::new(do_file::ten_loai(&f.loai)).weak());
                                ui.label(format!("{} câu", f.so));
                                ui.label(
                                    RichText::new(format!("Anh {:.0}%", f.anh * 100.0)).weak(),
                                );
                                ui.end_row();
                            }
                        });
                });
        });
        if quet {
            drop(v);
            self.chay("Đang dò file chữ", |v, bc| {
                let dung = v.viec.dung.clone();
                dung.store(false, Ordering::SeqCst);
                let kq = v.quet(&|s| bc.bao(s), &|| dung.load(Ordering::SeqCst));
                let n = kq.file.len();
                v.doc_chon();
                Ok(format!("Dò xong: {n} file chữ."))
            });
            return;
        }
        if can_doc_lai {
            v.luu_quet();
            v.doc_chon();
        }
        if v.chon.is_empty() {
            return;
        }

        // ---- dịch ----
        let (tong, da, con, ky_con) = v.thong_ke();
        let usd = v.viec.uoc_tinh_usd(ky_con);
        let mut lam: Option<&str> = None;
        the(ui, "DỊCH", |ui| {
            ui.label(format!(
                "{} file đã chọn • {tong} câu • đã dịch {da} • còn {con} câu ({ky_con} ký tự)",
                v.chon.len()
            ));
            if let Some(e) = &v.loi_doc {
                ui.label(RichText::new(format!("Lỗi đọc: {e}")).color(Color32::LIGHT_RED));
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !ban && con > 0 && dich::offline::co_the_dung(),
                        egui::Button::new("Dịch offline (miễn phí)"),
                    )
                    .clicked()
                {
                    lam = Some("offline");
                }
                if ui
                    .add_enabled(
                        !ban && con > 0 && dich::claude::co_khoa(),
                        egui::Button::new(format!("Dịch bằng Claude (~{usd:.2} USD)")),
                    )
                    .clicked()
                {
                    lam = Some("claude");
                }
                ui.label("Giới hạn mỗi lần:");
                for g in [1.0, 2.0, 5.0] {
                    ui.selectable_value(&mut self.gioi_han, g, format!("{g:.0} USD"));
                }
            });
            if !dich::claude::co_khoa() {
                ui.label(
                    RichText::new("Claude đang tắt hoặc chưa có mã API (Cài đặt > Mã API).")
                        .small()
                        .weak(),
                );
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Tự dịch bằng Excel:").small());
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!ban, egui::Button::new("Xuất CSV ra Desktop"))
                    .clicked()
                {
                    lam = Some("xuat");
                }
                if ui
                    .add_enabled(!ban, egui::Button::new("Nạp file đã dịch..."))
                    .clicked()
                {
                    lam = Some("nap");
                }
            });
        });

        // ---- ghi vào game ----
        let da_ghi = v.da_ghi();
        the(ui, "GHI VÀO GAME", |ui| {
            ui.checkbox(
                &mut self.khong_dau,
                "Không dấu (game không có font tiếng Việt)",
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !ban && da > 0,
                        egui::Button::new(RichText::new("Ghi vào game").strong()),
                    )
                    .clicked()
                {
                    lam = Some("ghi");
                }
                if !da_ghi.is_empty() {
                    if self.hoi_tra_lai {
                        ui.label(
                            RichText::new("Trả lại bản gốc cho mọi file đã ghi?")
                                .color(Color32::YELLOW),
                        );
                        if ui.button("Đồng ý").clicked() {
                            lam = Some("tra");
                            self.hoi_tra_lai = false;
                        }
                        if ui.button("Thôi").clicked() {
                            self.hoi_tra_lai = false;
                        }
                    } else if ui
                        .add_enabled(!ban, egui::Button::new("Trả lại bản gốc"))
                        .clicked()
                    {
                        self.hoi_tra_lai = true;
                    }
                }
            });
            ui.label(RichText::new("Lần đầu ghi sẽ sao lưu bản gốc; luôn dịch từ bản gốc nên ghi lại nhiều lần vẫn đúng.").small().weak());
            for (td, r) in &da_ghi {
                ui.label(
                    RichText::new(format!(
                        "Đã ghi {td}: {} câu lúc {}{}",
                        r.cau,
                        r.luc,
                        if r.khong_dau { " (không dấu)" } else { "" }
                    ))
                    .small(),
                );
            }
        });
        drop(v);

        let khong_dau = self.khong_dau;
        let gioi_han = self.gioi_han;
        match lam {
            Some("offline") => self.chay("Đang dịch offline", |v, bc| {
                let ds = v.viec.cau_can_dich(v.cac_cau().iter().map(String::as_str));
                let xong = v.viec.chay_offline(ds, bc).map_err(|e| e.to_string())?;
                Ok(if xong { "Dịch offline xong hết.".into() } else { format!("Đã dừng / còn câu lỗi ({} câu lỗi).", v.viec.loi) })
            }),
            Some("claude") => self.chay("Đang dịch bằng Claude", move |v, bc| {
                let ds = v.viec.cau_can_dich(v.cac_cau().iter().map(String::as_str));
                let xong = v.viec.chay_claude(ds, gioi_han, bc).map_err(|e| e.to_string())?;
                Ok(format!("{} • tốn {:.3} USD", if xong { "Dịch bằng Claude xong hết" } else { "Đã dừng – bấm lại để dịch tiếp" }, v.viec.tien))
            }),
            Some("xuat") => self.chay("Đang xuất CSV", move |v, _| {
                let ten: String = v.ten_game.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
                let p = duong_dan::desktop().join(format!("TuDich_{ten}.csv"));
                let n = v.xuat_csv(&p).map_err(|e| e.to_string())?;
                Ok(format!("Đã xuất {n} câu ra {} – mở bằng Excel, điền cột Tiếng Việt rồi bấm Nạp file đã dịch.", p.display()))
            }),
            Some("nap") => {
                if let Some(p) = rfd::FileDialog::new().add_filter("CSV", &["csv", "tsv", "txt"]).set_directory(duong_dan::desktop()).pick_file() {
                    self.chay("Đang nạp CSV", move |v, _| {
                        let n = v.nap_csv(&p).map_err(|e| e.to_string())?;
                        Ok(format!("Đã nạp {n} câu đã dịch."))
                    });
                }
            }
            Some("ghi") => self.chay("Đang ghi vào game", move |v, bc| {
                let n = v.ghi_vao_game(khong_dau, &|s| bc.bao(s)).map_err(|e| e.to_string())?;
                Ok(format!("Đã ghi {n} câu tiếng Việt vào game. Vào game kiểm tra; không ổn thì bấm Trả lại bản gốc."))
            }),
            Some("tra") => self.chay("Đang trả lại bản gốc", |v, _| {
                let n = v.tra_lai().map_err(|e| e.to_string())?;
                Ok(format!("Đã trả lại bản gốc cho {n} file."))
            }),
            _ => {}
        }
    }
}
