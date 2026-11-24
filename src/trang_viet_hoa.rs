//! Trang "Việt hóa game": thư viện game + Dò file chữ (giống app cũ, gọn hơn).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::viet_hoa::crimson_desert::{self as vcd, CrimsonDesert};
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
    /// Crimson Desert (công cụ riêng): đọc nền, None khi chưa đọc xong
    cd: Option<Arc<Mutex<Option<CrimsonDesert>>>>,
    cd_dot: usize,
    /// thống kê đã tính: ((đợt, số câu bộ nhớ), (tổng, đã, còn, ký tự còn), USD)
    cd_tk: Option<((usize, usize), (usize, usize, usize, usize), f64)>,
}

/// Chạy việc nền trên đối tượng dùng chung; tự cập nhật tiến trình.
fn chay_nen<T: Send + 'static>(
    tt: &Arc<Mutex<TienTrinh>>,
    doi_tuong: Arc<Mutex<T>>,
    ten: &str,
    viec: impl FnOnce(&mut T, &BaoTt) -> Result<String, String> + Send + 'static,
) {
    {
        let mut t = tt.lock().unwrap();
        if t.viec.is_some() {
            return;
        }
        *t = TienTrinh {
            viec: Some(ten.to_string()),
            thong_bao: format!("{ten}..."),
            ..Default::default()
        };
    }
    let tt = tt.clone();
    std::thread::spawn(move || {
        let bc = BaoTt(tt.clone());
        let kq = {
            let mut g = doi_tuong.lock().unwrap_or_else(|e| e.into_inner());
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
            cd: None,
            cd_dot: 0,
            cd_tk: None,
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
        chay_nen(&self.tt, v, ten, viec);
    }

    /// Chạy việc nền với Crimson Desert đã đọc xong.
    fn chay_cd(
        &self,
        ten: &str,
        viec: impl FnOnce(&mut CrimsonDesert, &BaoTt) -> Result<String, String> + Send + 'static,
    ) {
        let Some(o) = self.cd.clone() else { return };
        chay_nen(&self.tt, o, ten, move |cd, bc| match cd.as_mut() {
            Some(cd) => viec(cd, bc),
            None => Err("chưa đọc xong chữ của game".into()),
        });
    }

    fn mo_cd(&mut self, i: usize) {
        let thu_muc = self.games.as_ref().unwrap()[i].thu_muc.clone();
        self.chon = Some(i);
        self.viec = None;
        self.cd_tk = None;
        *self.tt.lock().unwrap() = TienTrinh::default();
        let o = Arc::new(Mutex::new(None));
        self.cd = Some(o.clone());
        chay_nen(
            &self.tt,
            o,
            "Đang đọc chữ tiếng Anh của game",
            move |cd, _| {
                let v = CrimsonDesert::nap(&thu_muc).map_err(|e| e.to_string())?;
                *cd = Some(v);
                Ok(String::new())
            },
        );
    }

    fn ve_cd(&mut self, ui: &mut egui::Ui) {
        let Some(o) = self.cd.clone() else { return };
        let ban = self.dang_ban();
        let Ok(g) = o.try_lock() else {
            ui.label(RichText::new("Crimson Desert: đang làm việc nền...").weak());
            return;
        };
        let Some(cd) = g.as_ref() else {
            if !ban {
                ui.label(
                    RichText::new("Chưa đọc được chữ của game (xem lỗi ở trên).")
                        .color(Color32::LIGHT_RED),
                );
            }
            return;
        };
        self.dung = cd.viec.dung.clone();
        let so_bo_nho = cd.viec.bo_nho.lock().map(|b| b.len()).unwrap_or(0);
        let khoa = (self.cd_dot, so_bo_nho);
        if self.cd_tk.as_ref().map(|t| t.0) != Some(khoa) && !ban {
            let tk = cd.thong_ke(self.cd_dot);
            self.cd_tk = Some((khoa, tk, cd.viec.uoc_tinh_usd(tk.3)));
        }
        let Some((_, (tong, da, con, ky_con), usd)) = self.cd_tk else {
            return;
        };
        let mut lam: Option<&str> = None;
        the(ui, "VIỆT HÓA CHỮ", |ui| {
            ui.horizontal(|ui| {
                for (i, d) in vcd::DOT.iter().enumerate() {
                    if ui
                        .add_enabled(
                            !ban,
                            egui::Button::new(format!("Đợt {}", i + 1)).selected(self.cd_dot == i),
                        )
                        .on_hover_text(d.ten)
                        .clicked()
                    {
                        self.cd_dot = i;
                    }
                }
            });
            ui.label(RichText::new(vcd::DOT[self.cd_dot].ten).strong());
            ui.label(format!(
                "{tong} câu khác nhau • đã dịch {da} • còn {con} câu ({ky_con} ký tự)"
            ));
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
            ui.label(RichText::new("Giữ nguyên tiếng Anh: tên nhân vật, tên riêng, tên vật phẩm. Câu đã dịch được lưu lại, lần sau chỉ dịch câu mới.").small().weak());
        });
        the(ui, "GÓI CHO DMM", |ui| {
            if ui
                .add_enabled(
                    !ban && so_bo_nho > 0,
                    egui::Button::new(RichText::new("Xuất gói Việt hóa ra Desktop").strong()),
                )
                .clicked()
            {
                lam = Some("goi");
            }
            ui.label(RichText::new("Game 2.04+ không cho sửa file trực tiếp: trong DMM tắt gói cũ > Import file VietHoa_CrimsonDesert.zip > bật > Mount Mods. Vào game để ngôn ngữ English.").small().weak());
        });
        the(ui, "MOD TĂNG Ô CHỨA ĐỒ", |ui| {
            if ui
                .add_enabled(!ban, egui::Button::new("Tạo mod tăng ô chứa đồ"))
                .clicked()
            {
                lam = Some("tui");
            }
            ui.label(RichText::new("Tạo 2 file ở Desktop\\Mod_TangO_CrimsonDesert: AnToan (balo 732, kho/tủ 1440 – nên dùng) và TatCa (cả balo 1440 – sao lưu save trước). Tắt game > DMM: Import 1 file, bật, Mount Mods.").small().weak());
        });
        ui.label(
            RichText::new("Lồng tiếng Crimson Desert sẽ có ở mục Lồng tiếng (giai đoạn 6).")
                .small()
                .weak(),
        );
        drop(g);

        let dot = self.cd_dot;
        let gioi_han = self.gioi_han;
        match lam {
            Some("offline") => self.chay_cd("Đang dịch offline", move |cd, bc| {
                let ds = cd.cau_can_dich(dot);
                let xong = cd.viec.chay_offline(ds, bc).map_err(|e| e.to_string())?;
                Ok(if xong {
                    format!("Dịch offline xong đợt {}. Bấm Xuất gói Việt hóa.", dot + 1)
                } else {
                    format!("Đã dừng / còn câu lỗi ({} câu lỗi).", cd.viec.loi)
                })
            }),
            Some("claude") => self.chay_cd("Đang dịch bằng Claude", move |cd, bc| {
                let ds = cd.cau_can_dich(dot);
                let xong = cd
                    .viec
                    .chay_claude(ds, gioi_han, bc)
                    .map_err(|e| e.to_string())?;
                Ok(format!(
                    "{} • tốn {:.3} USD",
                    if xong {
                        "Dịch bằng Claude xong đợt này"
                    } else {
                        "Đã dừng – bấm lại để dịch tiếp"
                    },
                    cd.viec.tien
                ))
            }),
            Some("goi") => self.chay_cd("Đang xuất gói cho DMM", |cd, _| {
                let p = duong_dan::desktop().join(vcd::TEN_GOI);
                let n = cd.xuat_goi(&p).map_err(|e| e.to_string())?;
                duong_dan::mo_explorer(&p);
                Ok(format!(
                    "Đã xuất gói ({n} câu tiếng Việt) ra Desktop: {}",
                    vcd::TEN_GOI
                ))
            }),
            Some("tui") => self.chay_cd("Đang tạo mod tăng ô chứa đồ", |cd, _| {
                let ra = duong_dan::desktop().join("Mod_TangO_CrimsonDesert");
                let bc = cd.tao_mod_tui_do(&ra).map_err(|e| e.to_string())?;
                duong_dan::mo_explorer(&ra);
                Ok(format!(
                    "Đã tạo mod ở Desktop\\Mod_TangO_CrimsonDesert:\n{}",
                    bc.join("\n")
                ))
            }),
            _ => {}
        }
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
        let mut mo_cd: Option<usize> = None;
        let mut tim_lai = false;
        let mut bo_tay: Option<PathBuf> = None;
        // Đã chọn game > thu gọn thư viện thành 1 dòng cho gọn
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
                self.cd = None;
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
                            let la_cd = g.loai() == LoaiGame::CrimsonDesert;
                            let rieng = match g.loai() {
                                LoaiGame::CrimsonDesert => None,
                                LoaiGame::Rise | LoaiGame::Tr2013 | LoaiGame::Shadow => {
                                    Some("Tomb Raider: công cụ riêng ở giai đoạn 5")
                                }
                                LoaiGame::Khac => None,
                            };
                            ui.horizontal(|ui| {
                                let chon = self.chon == Some(i);
                                if la_cd {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Việt hóa Crimson Desert").strong(),
                                            )
                                            .selected(chon),
                                        )
                                        .clicked()
                                    {
                                        mo_cd = Some(i);
                                    }
                                } else if ui
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
            self.cd = None;
            return;
        }
        if let Some(i) = mo_cd {
            if !self.dang_ban() {
                self.mo_cd(i);
            }
        }
        if let Some(i) = mo {
            if !self.dang_ban() {
                self.cd = None;
                let g = &self.games.as_ref().unwrap()[i];
                self.chon = Some(i);
                let mut v = ViecFile::moi(&g.thu_muc, &g.ten);
                v.doc_chon();
                self.dung = v.viec.dung.clone();
                self.viec = Some(Arc::new(Mutex::new(v)));
                *self.tt.lock().unwrap() = TienTrinh::default();
            }
        }

        if self.chon.is_none() {
            return;
        }
        if self.dang_ban() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(300));
        }
        self.ve_tien_trinh(ui);
        if self.cd.is_some() {
            self.ve_cd(ui);
            return;
        }
        let (Some(i), Some(viec)) = (self.chon, self.viec.clone()) else {
            return;
        };
        let ten_game = self.games.as_ref().unwrap()[i].ten.clone();
        let ban = self.dang_ban();

        let Ok(mut v) = viec.try_lock() else {
            ui.label(RichText::new(format!("{ten_game}: đang làm việc nền...")).weak());
            return;
        };
        self.ve_file(ui, &mut v, &ten_game, ban);
    }

    fn ve_tien_trinh(&mut self, ui: &mut egui::Ui) {
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
    }

    fn ve_file(&mut self, ui: &mut egui::Ui, v: &mut ViecFile, ten_game: &str, ban: bool) {
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
