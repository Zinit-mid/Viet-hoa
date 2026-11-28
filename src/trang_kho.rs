//! Trang Kho kiến thức: thêm / nhập file / sửa / bật tắt / xóa và thử xem AI sẽ được gửi gì.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::kho_kien_thuc::{self as kho, Loai, Muc};
use crate::viet_hoa::tim_game;

const DUNG_CHUNG: &str = "Dùng chung";

enum ViecMuc {
    DoiBat,
    Sua,
    Xoa,
}

pub struct TrangKho {
    /// None = chưa nạp
    muc: Option<Vec<Muc>>,
    /// file hỏng: không cho ghi đè
    hong: Option<String>,
    games: Arc<Mutex<Option<Vec<String>>>>,
    da_tim_game: bool,
    // ô thêm / sửa
    sua_id: Option<u64>,
    tieu_de: String,
    loai: Loai,
    game: String,
    noi_dung: String,
    o_truoc: String,
    kieu_go: crate::go_viet::Kieu,
    loc: Option<Loai>,
    cho_xoa: Option<u64>,
    thu: String,
    ket_qua_thu: String,
    thong_bao: String,
}

impl Default for TrangKho {
    fn default() -> Self {
        Self {
            muc: None,
            hong: None,
            games: Arc::new(Mutex::new(None)),
            da_tim_game: false,
            sua_id: None,
            tieu_de: String::new(),
            loai: Loai::CotTruyen,
            game: String::new(),
            noi_dung: String::new(),
            o_truoc: String::new(),
            kieu_go: crate::go_viet::Kieu::Tat,
            loc: None,
            cho_xoa: None,
            thu: String::new(),
            ket_qua_thu: String::new(),
            thong_bao: String::new(),
        }
    }
}

fn ten_game_hien(g: &str) -> &str {
    if g.is_empty() {
        DUNG_CHUNG
    } else {
        g
    }
}

impl TrangKho {
    /// Ghi `moi` xuống đĩa; chỉ nhận vào giao diện khi ghi được.
    fn ghi(&mut self, moi: Vec<Muc>, xong: String) -> bool {
        match kho::luu(&moi) {
            Ok(()) => {
                self.muc = Some(moi);
                self.thong_bao = xong;
                true
            }
            Err(e) => {
                self.thong_bao = format!("Lỗi lưu: {e}");
                false
            }
        }
    }

    fn xoa_o(&mut self) {
        self.sua_id = None;
        self.tieu_de.clear();
        self.noi_dung.clear();
        self.o_truoc.clear();
    }

    fn luu_muc(&mut self) {
        let mut ds = self.muc.clone().unwrap_or_default();
        let cu = self
            .sua_id
            .and_then(|id| ds.iter().position(|m| m.id == id));
        let m = Muc {
            id: cu
                .map(|i| ds[i].id)
                .unwrap_or_else(|| ds.iter().map(|m| m.id).max().unwrap_or(0) + 1),
            tieu_de: self.tieu_de.trim().to_string(),
            loai: self.loai,
            game: self.game.clone(),
            noi_dung: self.noi_dung.trim().to_string(),
            bat: cu.map(|i| ds[i].bat).unwrap_or(true),
        };
        if let Err(e) = kho::kiem_tra(&m) {
            self.thong_bao = format!("Chưa lưu: {e}");
            return;
        }
        match cu {
            Some(i) => ds[i] = m,
            None if ds.len() >= kho::SO_MUC_TOI_DA => {
                self.thong_bao = format!("Kho đã đủ {} mục.", kho::SO_MUC_TOI_DA);
                return;
            }
            None => ds.push(m),
        }
        if self.ghi(ds, "Đã lưu mục.".into()) {
            self.xoa_o();
        }
    }

    fn them_file(&mut self, files: Vec<std::path::PathBuf>) {
        let mut ds = self.muc.clone().unwrap_or_default();
        let mut id = ds.iter().map(|m| m.id).max().unwrap_or(0);
        let (mut them, mut loi) = (0usize, Vec::new());
        for f in files {
            let ten = f
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let kq = kho::doc_file(&f).and_then(|noi_dung| {
                let m = Muc {
                    id: id + 1,
                    tieu_de: ten.clone(),
                    loai: self.loai,
                    game: self.game.clone(),
                    noi_dung,
                    bat: true,
                };
                kho::kiem_tra(&m)?;
                Ok(m)
            });
            match kq {
                Ok(_) if ds.len() >= kho::SO_MUC_TOI_DA => loi.push(format!("{ten}: kho đã đầy")),
                Ok(m) => {
                    id += 1;
                    them += 1;
                    ds.push(m);
                }
                Err(e) => loi.push(format!("{ten}: {e}")),
            }
        }
        let mut tb = format!("Đã thêm {them} file.");
        if !loi.is_empty() {
            tb.push_str(&format!(" Không thêm được: {}", loi.join("; ")));
        }
        if them == 0 {
            self.thong_bao = tb;
        } else {
            self.ghi(ds, tb);
        }
    }

    fn lam(&mut self, ctx: &egui::Context, i: usize, viec: ViecMuc) {
        let Some(m) = self.muc.as_ref().and_then(|ds| ds.get(i)).cloned() else {
            return;
        };
        if let ViecMuc::Sua = viec {
            self.sua_id = Some(m.id);
            self.tieu_de = m.tieu_de;
            self.loai = m.loai;
            self.game = m.game;
            self.o_truoc = m.noi_dung.clone();
            self.noi_dung = m.noi_dung;
            crate::kieu::yeu_cau_cuon(ctx, "SỬA MỤC");
            return;
        }
        let mut moi = self.muc.clone().unwrap_or_default();
        if let ViecMuc::DoiBat = viec {
            moi[i].bat = !moi[i].bat;
            self.ghi(moi, "Đã lưu.".into());
        } else {
            moi.remove(i);
            if self.ghi(moi, "Đã xóa mục.".into()) && self.sua_id == Some(m.id) {
                self.xoa_o();
            }
            self.cho_xoa = None;
        }
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Kho kiến thức");
        ui.label("Nạp kiến thức cho AI local. App không huấn luyện lại AI: mỗi lần hỏi hoặc dịch, app tìm đoạn liên quan rồi gửi kèm.");
        ui.add_space(8.0);
        if self.muc.is_none() && self.hong.is_none() {
            match kho::nap() {
                Ok(m) => self.muc = Some(m),
                Err(e) => self.hong = Some(e.to_string()),
            }
        }
        if let Some(e) = self.hong.clone() {
            ui.label(RichText::new(format!("{e}. App không ghi đè file này. Bản trước đó (nếu có) là kho_kien_thuc.json.bak trong thư mục dữ liệu.")).color(Color32::LIGHT_RED));
            if ui.button("Đọc lại").clicked() {
                self.hong = None;
            }
            return;
        }
        if !self.da_tim_game {
            self.da_tim_game = true;
            let o = self.games.clone();
            std::thread::spawn(move || {
                let mut ds: Vec<String> = tim_game::tim_tat_ca()
                    .iter()
                    .map(|g| kho::ten_game(g.loai(), &g.ten))
                    .collect();
                ds.sort();
                ds.dedup();
                *o.lock().unwrap_or_else(|e| e.into_inner()) = Some(ds);
            });
        }
        let games = self.games.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if games.is_none() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }
        let games = games.unwrap_or_default();

        let tieu_de_khoi = if self.sua_id.is_some() {
            "SỬA MỤC"
        } else {
            "THÊM MỤC"
        };
        crate::kieu::khoi(ui, tieu_de_khoi, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Loại:");
                for l in Loai::TAT_CA {
                    ui.selectable_value(&mut self.loai, l, l.ten());
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Dành cho:");
                egui::ComboBox::from_id_salt("kho_game")
                    .selected_text(ten_game_hien(&self.game))
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.game, String::new(), DUNG_CHUNG);
                        for g in &games {
                            ui.selectable_value(&mut self.game, g.clone(), g.as_str());
                        }
                    });
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Tiêu đề:");
                ui.add(egui::TextEdit::singleline(&mut self.tieu_de).desired_width(420.0));
            });
            ui.label(RichText::new(self.loai.goi_y()).small().weak());
            let r = egui::ScrollArea::vertical()
                .id_salt("kho_noi_dung")
                .max_height(260.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.noi_dung)
                            .desired_rows(8)
                            .desired_width(f32::INFINITY)
                            .hint_text("Dán hoặc gõ nội dung vào đây"),
                    )
                })
                .inner;
            if r.changed() {
                let truoc = self.noi_dung.clone();
                crate::go_viet::ap_dung(&mut self.noi_dung, &mut self.o_truoc, self.kieu_go);
                if truoc != self.noi_dung {
                    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                        let n = self.noi_dung.chars().count();
                        st.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(n),
                        )));
                        st.store(ui.ctx(), r.id);
                    }
                }
            } else {
                self.o_truoc = self.noi_dung.clone();
            }
            ui.horizontal_wrapped(|ui| {
                use crate::go_viet::Kieu;
                ui.label("Gõ tiếng Việt:");
                ui.radio_value(&mut self.kieu_go, Kieu::Tat, "Tắt (dán chữ / dùng Unikey)");
                ui.radio_value(&mut self.kieu_go, Kieu::Vni, "VNI");
                ui.radio_value(&mut self.kieu_go, Kieu::Telex, "Telex");
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Lưu mục").clicked() {
                    self.luu_muc();
                }
                if ui.button("Xóa trắng / Hủy sửa").clicked() {
                    self.xoa_o();
                }
                if ui.button("Thêm file .txt / .md...").clicked() {
                    if let Some(f) = rfd::FileDialog::new()
                        .add_filter("Văn bản", &["txt", "md"])
                        .pick_files()
                    {
                        self.them_file(f);
                    }
                }
            });
            ui.label(RichText::new("File thêm vào dùng Loại và Dành cho đang chọn ở trên. Đừng nạp mật khẩu hay mã API.").small().weak());
            if !self.thong_bao.is_empty() {
                ui.label(RichText::new(&self.thong_bao).color(Color32::YELLOW));
            }
        });

        crate::kieu::khoi(ui, "KHO ĐÃ CÓ", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.loc, None, "Tất cả");
                for l in Loai::TAT_CA {
                    ui.selectable_value(&mut self.loc, Some(l), l.ten());
                }
            });
            let ds = self.muc.as_deref().unwrap_or_default();
            if ds.is_empty() {
                ui.label("Kho đang trống.");
            }
            // chỉ ghi nhận thao tác trong lúc vẽ, làm sau (khỏi sao chép cả kho mỗi khung hình)
            let mut viec = None;
            for (i, m) in ds.iter().enumerate() {
                if self.loc.is_some_and(|l| l != m.loai) {
                    continue;
                }
                ui.push_id(m.id, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        let mut bat = m.bat;
                        if ui.checkbox(&mut bat, "Bật").changed() {
                            viec = Some((i, ViecMuc::DoiBat));
                        }
                        ui.label(RichText::new(&m.tieu_de).strong());
                        ui.label(
                            RichText::new(format!("{} • {}", m.loai.ten(), ten_game_hien(&m.game)))
                                .weak(),
                        );
                        if ui.button("Sửa").clicked() {
                            viec = Some((i, ViecMuc::Sua));
                        }
                        if self.cho_xoa == Some(m.id) {
                            if ui.button("Xóa thật").clicked() {
                                viec = Some((i, ViecMuc::Xoa));
                            }
                            if ui.button("Thôi").clicked() {
                                self.cho_xoa = None;
                            }
                        } else if ui.button("Xóa").clicked() {
                            self.cho_xoa = Some(m.id);
                        }
                    });
                });
            }
            if let Some((i, v)) = viec {
                self.lam(ui.ctx(), i, v);
            }
        });

        crate::kieu::khoi(ui, "THỬ TÌM", |ui| {
            ui.label("Gõ một câu hỏi hoặc dán một câu thoại tiếng Anh để xem đoạn nào sẽ được gửi cho AI khi trò chuyện.");
            ui.add(egui::TextEdit::singleline(&mut self.thu).desired_width(f32::INFINITY));
            if ui.button("Tìm thử").clicked() {
                let (chu, _) = kho::cho_cau_hoi(&self.thu);
                self.ket_qua_thu = if chu.is_empty() {
                    "Không có đoạn nào liên quan.".into()
                } else {
                    chu
                };
            }
            if !self.ket_qua_thu.is_empty() {
                ui.add(egui::Label::new(self.ket_qua_thu.as_str()).wrap());
            }
        });
    }
}
