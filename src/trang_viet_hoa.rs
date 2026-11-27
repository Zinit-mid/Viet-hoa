//! Trang "Việt hóa game": thư viện game + Dò file chữ (giống app cũ, gọn hơn).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::viet_hoa::crimson_desert::{self as vcd, CrimsonDesert};
use crate::viet_hoa::do_file::{self, ViecFile};
use crate::viet_hoa::tim_game::{self, Game, LoaiGame};
use crate::viet_hoa::tomb_raider::{LoaiTr, TombRaider};
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
    /// Tomb Raider (Rise / 2013 / Shadow)
    tr: Option<Arc<Mutex<Option<TombRaider>>>>,
    /// thống kê đã tính: (số câu bộ nhớ, (dòng, tổng, đã, còn, ký tự), (phụ đề, đã dịch, đã lồng), USD, đã cài)
    tr_tk: Option<(usize, TkTr)>,
    hoi_go: bool,
    /// cờ dừng hiện tại (cho thanh tiến trình toàn cục)
    dung_g: Arc<Mutex<Arc<AtomicBool>>>,
}

type TkTr = (
    (usize, usize, usize, usize, usize),
    (usize, usize, usize),
    f64,
    bool,
);

fn loai_tr(l: LoaiGame) -> Option<LoaiTr> {
    match l {
        LoaiGame::Rise => Some(LoaiTr::Rise),
        LoaiGame::Tr2013 => Some(LoaiTr::Tr2013),
        LoaiGame::Shadow => Some(LoaiTr::Shadow),
        _ => None,
    }
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
            tr: None,
            tr_tk: None,
            hoi_go: false,
            dung_g: Arc::new(Mutex::new(Arc::new(AtomicBool::new(false)))),
        }
    }
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    crate::kieu::khoi(ui, tieu_de, noi);
}

impl TrangVietHoa {
    fn tim_lai(&mut self) {
        self.games = None;
        let o = self.dang_tim.clone();
        std::thread::spawn(move || {
            *o.lock().unwrap() = Some(tim_game::tim_tat_ca());
        });
    }

    pub fn dang_ky_tt(&self) {
        let t = self.tt.clone();
        crate::tien_trinh::dang_ky("viet_hoa", Some(self.dung_g.clone()), move || {
            let g = t.lock().unwrap();
            crate::tien_trinh::Anh {
                dang_ban: g.viec.is_some(),
                ten: g.viec.clone().unwrap_or_else(|| "Việt hóa game".into()),
                thong_bao: g.thong_bao.clone(),
                tien: (g.tong > 0).then(|| g.xong as f32 / g.tong as f32),
                loi: g.thong_bao.starts_with("Lỗi"),
            }
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

    fn chay_tr(
        &self,
        ten: &str,
        viec: impl FnOnce(&mut TombRaider, &BaoTt) -> Result<String, String> + Send + 'static,
    ) {
        let Some(o) = self.tr.clone() else { return };
        chay_nen(&self.tt, o, ten, move |x, bc| match x.as_mut() {
            Some(x) => viec(x, bc),
            None => Err("chưa đọc xong chữ của game".into()),
        });
    }

    fn mo_tr(&mut self, i: usize) {
        let g = &self.games.as_ref().unwrap()[i];
        let (thu_muc, Some(loai)) = (g.thu_muc.clone(), loai_tr(g.loai())) else {
            return;
        };
        self.chon = Some(i);
        self.viec = None;
        self.tr_tk = None;
        self.hoi_go = false;
        *self.tt.lock().unwrap() = TienTrinh::default();
        let o = Arc::new(Mutex::new(None));
        self.tr = Some(o.clone());
        let dung = Arc::new(AtomicBool::new(false));
        self.dung = dung.clone();
        *self.dung_g.lock().unwrap() = self.dung.clone();
        chay_nen(
            &self.tt,
            o,
            "Đang đọc chữ của game (lần đầu đọc phụ đề phim mất 1–3 phút)",
            move |x, bc| {
                let v = TombRaider::nap(&thu_muc, loai, &|s| bc.bao(s), &dung)
                    .map_err(|e| e.to_string())?;
                *x = Some(v);
                Ok(String::new())
            },
        );
    }

    fn ve_tr(&mut self, ui: &mut egui::Ui) {
        let Some(o) = self.tr.clone() else { return };
        let ban = self.dang_ban();
        let Ok(g) = o.try_lock() else {
            ui.label(RichText::new("Tomb Raider: đang làm việc nền...").weak());
            return;
        };
        let Some(x) = g.as_ref() else {
            if !ban {
                ui.label(
                    RichText::new("Chưa đọc được chữ của game (xem lỗi ở trên).")
                        .color(Color32::LIGHT_RED),
                );
            }
            return;
        };
        self.dung = x.viec.dung.clone();
        *self.dung_g.lock().unwrap() = self.dung.clone();
        let so_bo_nho = x.viec.bo_nho.lock().map(|b| b.len()).unwrap_or(0);
        if !ban && self.tr_tk.as_ref().map(|t| t.0) != Some(so_bo_nho) {
            let tk = x.thong_ke();
            self.tr_tk = Some((
                so_bo_nho,
                (
                    tk,
                    x.thong_ke_phu_de(),
                    x.viec.uoc_tinh_usd(tk.4),
                    x.da_cai(),
                ),
            ));
        }
        let Some((_, ((dong, tong, da, con, ky), (pd, pd_da, long), usd, da_cai))) = self.tr_tk
        else {
            return;
        };
        let loai = x.loai;
        let mut lam: Option<&str> = None;
        the(
            ui,
            &format!("VIỆT HÓA {}", loai.ten().to_uppercase()),
            |ui| {
                if x.co_viet_hoa_san {
                    ui.label(RichText::new("Game đã có sẵn bản Việt hóa chữ (của nhóm khác) – không cần dịch / cài chữ.").color(Color32::LIGHT_GREEN));
                }
                ui.label(format!("{dong} dòng chữ • {tong} câu cần dịch • đã dịch {da} • còn {con} câu ({ky} ký tự)"));
                if loai != LoaiTr::Shadow {
                    ui.label(format!("Phụ đề phim / lời thoại: {pd} câu, đã dịch {pd_da} (gộp trong bước Dịch) • {long} đoạn đã lồng tiếng"));
                }
                if let Some(e) = &x.loi_phu_de {
                    ui.label(
                        RichText::new(format!("Không đọc được phụ đề phim: {e}"))
                            .color(Color32::YELLOW),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if crate::kieu::nut_tien_do(
                        ui,
                        "viet_hoa",
                        "Dịch offline (miễn phí)",
                        "Đang dịch",
                        crate::kieu::TIM,
                        !ban && con > 0 && dich::offline::co_the_dung(),
                    ) {
                        lam = Some("offline");
                    }
                    if crate::kieu::nut_tien_do(
                        ui,
                        "viet_hoa",
                        &format!("Dịch bằng Claude (~{usd:.2} USD)"),
                        "Đang dịch",
                        crate::kieu::TIM,
                        !ban && con > 0 && dich::claude::co_khoa(),
                    ) {
                        lam = Some("claude");
                    }
                    if crate::kieu::nut_tien_do(
                        ui,
                        "viet_hoa",
                        "AI local sửa câu dịch dở",
                        "Đang soát",
                        crate::kieu::TIM,
                        !ban,
                    ) {
                        lam = Some("ai_sua");
                    }
                    ui.label("Giới hạn mỗi lần:");
                    for g in [1.0, 2.0, 5.0] {
                        ui.selectable_value(&mut self.gioi_han, g, format!("{g:.0} USD"));
                    }
                });
                ui.label(RichText::new("Giữ nguyên tiếng Anh: tên người, địa danh, tổ chức. Câu đã dịch được lưu lại, lần sau chỉ dịch câu mới.").small().weak());
            },
        );
        the(ui, "CÀI VÀO GAME", |ui| {
            ui.label(if da_cai {
                RichText::new("Đang cài trong game: CÓ bản Việt hóa").color(Color32::LIGHT_GREEN)
            } else {
                RichText::new("Đang cài trong game: chưa")
            });
            ui.horizontal_wrapped(|ui| {
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Cài Việt hóa (có dấu)",
                    "Đang cài",
                    crate::kieu::TIM,
                    !ban && da > 0,
                ) {
                    lam = Some("cai");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Cài (không dấu)",
                    "Đang cài",
                    crate::kieu::TIM,
                    !ban && da > 0,
                ) {
                    lam = Some("cai_kd");
                }
                if da_cai {
                    if self.hoi_go {
                        ui.label(
                            RichText::new("Gỡ Việt hóa, game trở lại tiếng Anh?")
                                .color(Color32::YELLOW),
                        );
                        if ui.button("Đồng ý").clicked() {
                            lam = Some("go");
                            self.hoi_go = false;
                        }
                        if ui.button("Thôi").clicked() {
                            self.hoi_go = false;
                        }
                    } else if ui
                        .add_enabled(!ban, egui::Button::new("Gỡ Việt hóa"))
                        .clicked()
                    {
                        self.hoi_go = true;
                    }
                }
            });
            let cach = match loai {
                LoaiTr::Rise => "Tạo gói mod riêng trong thư mục game (chữ, phụ đề phim, lời thoại, giọng đã lồng, font tiếng Việt) – không sửa file gốc. Gỡ = xóa gói.",
                LoaiTr::Tr2013 => "Tạo lại gói patch2.000.tiger (bản gốc giữ ở bigfile._orig.patch2.000.tiger). Gỡ = trả lại bản gốc.",
                LoaiTr::Shadow => "Tạo gói mod riêng trong thư mục game – không sửa file gốc. Gỡ = xóa gói.",
            };
            ui.label(RichText::new(format!("{cach}\nTẮT GAME trước khi cài / gỡ. Trong game để ngôn ngữ English. Font game thiếu dấu > dùng \"không dấu\".")).small().weak());
        });
        if loai != LoaiTr::Shadow {
            the(ui, "LỒNG TIẾNG VIỆT (VieNeu)", |ui| {
                let co = crate::viet_hoa::giong_vieneu::tim_vieneu().is_some();
                ui.label(format!("{long} đoạn đã lồng tiếng. Giọng đã lồng bằng app cũ vẫn dùng được và được cài kèm."));
                if !co {
                    ui.label(
                        RichText::new(
                            "Không thấy app VieNeu – mở VieNeu, cài bản GPU rồi quay lại.",
                        )
                        .color(Color32::YELLOW),
                    );
                }
                ui.horizontal_wrapped(|ui| {
                    if crate::kieu::nut_tien_do(
                        ui,
                        "viet_hoa",
                        "Lồng thử ~16 đoạn đầu game",
                        "Đang lồng",
                        crate::kieu::CAM,
                        !ban && co && pd_da > 0,
                    ) {
                        lam = Some("long_thu");
                    }
                    if crate::kieu::nut_tien_do(
                        ui,
                        "viet_hoa",
                        "Lồng tiếng toàn bộ",
                        "Đang lồng",
                        crate::kieu::CAM,
                        !ban && co && pd_da > 0,
                    ) {
                        lam = Some("long");
                    }
                });
                ui.label(RichText::new("Cần dịch chữ trước. Tắt game, đóng app VieNeu, laptop thì cắm sạc. Chạy lại = làm tiếp phần còn thiếu. Lồng xong bấm Cài Việt hóa để ghi giọng vào game.").small().weak());
            });
        }
        drop(g);

        let gioi_han = self.gioi_han;
        match lam {
            Some("ai_sua") => self.chay_tr("AI local đang soát bản dịch", |x, bc| {
                let cap = x.viec.cau_nghi_van(400);
                let n = x.viec.ai_sua(cap, bc).map_err(|e| e.to_string())?;
                Ok(format!("AI đã sửa {n} câu dịch dở. Bấm lại để soát tiếp phần còn lại, rồi Cài Việt hóa."))
            }),
            Some("offline") => self.chay_tr("Đang dịch offline", |x, bc| {
                let ds = x.cau_can_dich();
                let ok = x.viec.chay_offline(ds, bc).map_err(|e| e.to_string())?;
                Ok(if ok {
                    "Dịch offline xong. Bấm Cài Việt hóa (tắt game trước).".into()
                } else {
                    format!("Chưa dịch hết: còn {} câu (đã thử lại, danh sách ở cau_chua_dich.txt trong thư mục dữ liệu game). Bấm dịch lần nữa để thử tiếp.", x.viec.loi)
                })
            }),
            Some("claude") => self.chay_tr("Đang dịch bằng Claude", move |x, bc| {
                let ds = x.cau_can_dich();
                let ok = x
                    .viec
                    .chay_claude(ds, gioi_han, bc)
                    .map_err(|e| e.to_string())?;
                Ok(format!(
                    "{} • tốn {:.3} USD",
                    if ok {
                        "Dịch bằng Claude xong"
                    } else {
                        "Chưa dịch hết (bị dừng, chạm giới hạn USD hoặc còn câu hỏng, xem nhật ký) – bấm lại để dịch tiếp"
                    },
                    x.viec.tien
                ))
            }),
            Some(k @ ("cai" | "cai_kd")) => {
                let kd = k == "cai_kd";
                self.tr_tk = None;
                self.chay_tr(
                    "Đang cài vào game (tắt game trước)",
                    move |x, bc| {
                        let _thuc = duong_dan::GiuMayThuc::moi();
                        let dung = x.viec.dung.clone();
                        dung.store(false, Ordering::SeqCst);
                        x.cai(kd, &|s| bc.bao(s), &dung)
                            .map_err(|e| e.to_string())?;
                        Ok(format!("{} Mở game để chơi.", x.ket_qua_cai))
                    },
                );
            }
            Some(k @ ("long_thu" | "long")) => {
                let thu = k == "long_thu";
                self.tr_tk = None;
                self.chay_tr(
                    if thu { "Đang lồng thử" } else { "Đang lồng tiếng" },
                    move |x, bc| {
                        let _thuc = duong_dan::GiuMayThuc::moi();
                        let _ai = crate::ai_local::KhoaLongTieng::vao().map_err(|e| e.to_string())?;
                        let dung = x.viec.dung.clone();
                        dung.store(false, Ordering::SeqCst);
                        let kq = x
                            .long_tieng(
                                thu,
                                4.0,
                                &dung,
                                &|s| bc.bao(s),
                                &|a, b, s| {
                                    bc.tien_do(a, b, 0.0);
                                    bc.bao(&format!("{s} ({a}/{b})"));
                                },
                            )
                            .map_err(|e| e.to_string())?;
                        Ok(match kq {
                            None => "Đã dừng – bấm lại để làm tiếp.".to_string(),
                            Some(k) => format!(
                                "Lồng xong {} file mới • tổng {}/{} file đã có giọng Việt. Bấm Cài Việt hóa để ghi vào game.",
                                k.file, k.da_co, k.tong
                            ),
                        })
                    },
                );
            }
            Some("go") => {
                self.tr_tk = None;
                self.chay_tr("Đang gỡ Việt hóa", |x, _| {
                    x.go().map_err(|e| e.to_string())?;
                    Ok("Đã gỡ Việt hóa – game trở lại tiếng Anh.".into())
                });
            }
            _ => {}
        }
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
        *self.dung_g.lock().unwrap() = self.dung.clone();
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
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Dịch offline (miễn phí)",
                    "Đang dịch",
                    crate::kieu::TIM,
                    !ban && con > 0 && dich::offline::co_the_dung(),
                ) {
                    lam = Some("offline");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    &format!("Dịch bằng Claude (~{usd:.2} USD)"),
                    "Đang dịch",
                    crate::kieu::TIM,
                    !ban && con > 0 && dich::claude::co_khoa(),
                ) {
                    lam = Some("claude");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "AI local sửa câu dịch dở",
                    "Đang soát",
                    crate::kieu::TIM,
                    !ban,
                ) {
                    lam = Some("ai_sua");
                }
                ui.label("Giới hạn mỗi lần:");
                for g in [1.0, 2.0, 5.0] {
                    ui.selectable_value(&mut self.gioi_han, g, format!("{g:.0} USD"));
                }
            });
            ui.label(RichText::new("Giữ nguyên tiếng Anh: tên nhân vật, tên riêng, tên vật phẩm. Câu đã dịch được lưu lại, lần sau chỉ dịch câu mới.").small().weak());
        });
        the(ui, "GÓI CHO DMM", |ui| {
            if crate::kieu::nut_tien_do(
                ui,
                "viet_hoa",
                "Xuất gói Việt hóa ra Desktop",
                "Đang xuất",
                crate::kieu::TIM,
                !ban && so_bo_nho > 0,
            ) {
                lam = Some("goi");
            }
            ui.label(RichText::new("Game 2.04+ không cho sửa file trực tiếp: trong DMM tắt gói cũ > Import file VietHoa_CrimsonDesert.zip > bật > Mount Mods. Vào game để ngôn ngữ English.").small().weak());
        });
        the(ui, "MOD TĂNG Ô CHỨA ĐỒ", |ui| {
            if crate::kieu::nut_tien_do(
                ui,
                "viet_hoa",
                "Tạo mod tăng ô chứa đồ",
                "Đang tạo",
                crate::kieu::TIM,
                !ban,
            ) {
                lam = Some("tui");
            }
            ui.label(RichText::new("Tạo 2 file ở Desktop\\Mod_TangO_CrimsonDesert: AnToan (balo 732, kho/tủ 1440 – nên dùng) và TatCa (cả balo 1440 – sao lưu save trước). Tắt game > DMM: Import 1 file, bật, Mount Mods.").small().weak());
        });
        the(ui, "LỒNG TIẾNG VIỆT (VieNeu + Wwise)", |ui| {
            let co = crate::viet_hoa::giong_vieneu::tim_vieneu().is_some()
                && crate::viet_hoa::wwise::tim_wwise_console().is_some();
            if !co {
                ui.label(
                    RichText::new("Cần app VieNeu (bản GPU) và Wwise (WwiseConsole.exe) trên máy.")
                        .color(Color32::YELLOW),
                );
            }
            ui.horizontal_wrapped(|ui| {
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Lồng nhân vật có tên (A)",
                    "Đang lồng",
                    crate::kieu::CAM,
                    !ban && co && so_bo_nho > 0,
                ) {
                    lam = Some("long_a");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Lồng dân thường (B)",
                    "Đang lồng",
                    crate::kieu::CAM,
                    !ban && co && so_bo_nho > 0,
                ) {
                    lam = Some("long_b");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Sao lưu lồng tiếng ra thư mục...",
                    "Đang sao lưu",
                    crate::kieu::XANH_BIEN,
                    !ban,
                ) {
                    lam = Some("sao_luu");
                }
            });
            ui.label(RichText::new("Cần dịch chữ trước (các đợt). Giọng mẫu lấy từ chính giọng gốc của từng người (hoặc giọng riêng trong giong_rieng\\phan_bo.json). Rất lâu và nặng (hàng chục GB): tắt game, đóng VieNeu, cắm sạc. Chạy lại = làm tiếp. Xong ra Desktop file LongTieng_*.zip > DMM Import, bật, Mount Mods.").small().weak());
        });
        drop(g);

        let dot = self.cd_dot;
        let gioi_han = self.gioi_han;
        match lam {
            Some("ai_sua") => self.chay_cd("AI local đang soát bản dịch", |cd, bc| {
                let cap = cd.viec.cau_nghi_van(400);
                let n = cd.viec.ai_sua(cap, bc).map_err(|e| e.to_string())?;
                Ok(format!("AI đã sửa {n} câu dịch dở. Bấm lại để soát tiếp phần còn lại, rồi Xuất gói Việt hóa."))
            }),
            Some("offline") => self.chay_cd("Đang dịch offline", move |cd, bc| {
                let ds = cd.cau_can_dich(dot);
                let xong = cd.viec.chay_offline(ds, bc).map_err(|e| e.to_string())?;
                Ok(if xong {
                    format!("Dịch offline xong đợt {}. Bấm Xuất gói Việt hóa.", dot + 1)
                } else {
                    format!("Chưa dịch hết: còn {} câu (đã thử lại, danh sách ở cau_chua_dich.txt trong thư mục dữ liệu game). Bấm dịch lần nữa để thử tiếp.", cd.viec.loi)
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
                        "Chưa dịch hết (bị dừng, chạm giới hạn USD hoặc còn câu hỏng, xem nhật ký) – bấm lại để dịch tiếp"
                    },
                    cd.viec.tien
                ))
            }),
            Some(k @ ("long_a" | "long_b")) => {
                let phan = if k == "long_a" { 'A' } else { 'B' };
                self.chay_cd("Đang lồng tiếng", move |cd, bc| {
                    let _thuc = duong_dan::GiuMayThuc::moi();
                    let _ai = crate::ai_local::KhoaLongTieng::vao().map_err(|e| e.to_string())?;
                    let dung = cd.viec.dung.clone();
                    dung.store(false, Ordering::SeqCst);
                    let lv = crate::viet_hoa::tomb_raider::thu_muc_lam_viec(&cd.thu_muc_game);
                    let ten = if phan == 'A' { "LongTieng_NhanVat.zip" } else { "LongTieng_DanThuong.zip" };
                    let zip = duong_dan::desktop().join(ten);
                    let kq = cd
                        .long_tieng_phan(phan, &lv, &zip, 6.0, &dung, &|s| bc.bao(s), &|a, b, s| {
                            bc.tien_do(a, b, 0.0);
                            bc.bao(&format!("{s} ({a}/{b})"));
                        })
                        .map_err(|e| e.to_string())?;
                    Ok(match kq {
                        None => "Đã dừng – bấm lại để làm tiếp.".to_string(),
                        Some(k) => {
                            duong_dan::mo_explorer(&zip);
                            format!(
                                "Xong {} câu của {} giọng (thiếu giọng mẫu: {}). Gói: Desktop\\{ten}",
                                k.cau, k.giong, k.thieu_mau.len()
                            )
                        }
                    })
                });
            }
            Some("sao_luu") => {
                if let Some(dich) = rfd::FileDialog::new().set_title("Chọn thư mục để sao lưu lồng tiếng").pick_folder() {
                    self.chay_cd("Đang sao lưu lồng tiếng", move |cd, bc| {
                        let lv = crate::viet_hoa::tomb_raider::thu_muc_lam_viec(&cd.thu_muc_game);
                        let ra = dich.join("SaoLuu_LongTieng");
                        let mut so = 0usize;
                        let mut ghi: Vec<String> = Vec::new();
                        let mut cho: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
                        for t in ["LongTieng_NhanVat.zip", "LongTieng_DanThuong.zip"] {
                            let nguon = duong_dan::desktop().join(t);
                            if nguon.is_file() {
                                cho.push((nguon, ra.join(t)));
                                ghi.push(t.to_string());
                            }
                        }
                        for ph in ["A", "B"] {
                            for e in std::fs::read_dir(lv.join(ph)).into_iter().flatten().flatten() {
                                let ten = e.file_name().to_string_lossy().to_string();
                                if ten.starts_with("_wem") && e.path().is_dir() {
                                    let mut c = vec![e.path()];
                                    while let Some(d) = c.pop() {
                                        for f in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                                            let fp = f.path();
                                            if fp.is_dir() {
                                                c.push(fp);
                                            } else if let Ok(rel) = fp.strip_prefix(lv.join(ph)) {
                                                cho.push((fp.clone(), ra.join(ph).join(rel)));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let tong = cho.len();
                        if tong == 0 {
                            return Err("chưa có file lồng tiếng nào để sao lưu (lồng xong mới có)".into());
                        }
                        for (i, (a, b)) in cho.into_iter().enumerate() {
                            if let Some(d) = b.parent() {
                                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                            }
                            std::fs::copy(&a, &b).map_err(|e| format!("{}: {e}", a.display()))?;
                            so += 1;
                            if i % 200 == 0 {
                                bc.tien_do(i, tong, 0.0);
                                bc.bao(&format!("Đang chép {i}/{tong}"));
                            }
                        }
                        duong_dan::mo_explorer(&ra);
                        Ok(format!("Đã sao lưu {so} file vào {} (gói zip: {})", ra.display(), if ghi.is_empty() { "chưa có".into() } else { ghi.join(", ") }))
                    });
                }
            }
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
        let mut mo_tr: Option<usize> = None;
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
                self.tr = None;
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
                            let la_tr = loai_tr(g.loai());
                            let rieng: Option<&str> = None;
                            ui.horizontal(|ui| {
                                let chon = self.chon == Some(i);
                                if la_tr.is_some() {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Việt hóa Tomb Raider").strong(),
                                            )
                                            .selected(chon),
                                        )
                                        .clicked()
                                    {
                                        mo_tr = Some(i);
                                    }
                                } else if la_cd {
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
            self.tr = None;
            return;
        }
        if let Some(i) = mo_cd {
            if !self.dang_ban() {
                self.tr = None;
                self.mo_cd(i);
            }
        }
        if let Some(i) = mo_tr {
            if !self.dang_ban() {
                self.cd = None;
                self.mo_tr(i);
            }
        }
        if let Some(i) = mo {
            if !self.dang_ban() {
                self.cd = None;
                self.tr = None;
                let g = &self.games.as_ref().unwrap()[i];
                self.chon = Some(i);
                let mut v = ViecFile::moi(&g.thu_muc, &g.ten);
                v.doc_chon();
                self.dung = v.viec.dung.clone();
                *self.dung_g.lock().unwrap() = self.dung.clone();
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
        if self.tr.is_some() {
            self.ve_tr(ui);
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
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Dịch offline (miễn phí)",
                    "Đang dịch",
                    crate::kieu::TIM,
                    !ban && con > 0 && dich::offline::co_the_dung(),
                ) {
                    lam = Some("offline");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    &format!("Dịch bằng Claude (~{usd:.2} USD)"),
                    "Đang dịch",
                    crate::kieu::TIM,
                    !ban && con > 0 && dich::claude::co_khoa(),
                ) {
                    lam = Some("claude");
                }
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "AI local sửa câu dịch dở",
                    "Đang soát",
                    crate::kieu::TIM,
                    !ban,
                ) {
                    lam = Some("ai_sua");
                }
                ui.label("Giới hạn mỗi lần:");
                for g in [1.0, 2.0, 5.0] {
                    ui.selectable_value(&mut self.gioi_han, g, format!("{g:.0} USD"));
                }
            });
            if let Some(ly) = dich::claude::ly_do_khong_dung() {
                ui.label(RichText::new(ly).small().color(Color32::YELLOW));
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
            crate::kieu::cong_tac(
                ui,
                &mut self.khong_dau,
                "Không dấu (game không có font tiếng Việt)",
            );
            ui.horizontal_wrapped(|ui| {
                if crate::kieu::nut_tien_do(
                    ui,
                    "viet_hoa",
                    "Ghi vào game",
                    "Đang ghi",
                    crate::kieu::TIM,
                    !ban && da > 0,
                ) {
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
            Some("ai_sua") => self.chay("AI local đang soát bản dịch", |v, bc| {
                let cap = v.viec.cau_nghi_van(400);
                let n = v.viec.ai_sua(cap, bc).map_err(|e| e.to_string())?;
                Ok(format!("AI đã sửa {n} câu dịch dở. Bấm lại để soát tiếp phần còn lại, rồi Ghi vào game."))
            }),
            Some("offline") => self.chay("Đang dịch offline", |v, bc| {
                let ds = v.viec.cau_can_dich(v.cac_cau().iter().map(String::as_str));
                let xong = v.viec.chay_offline(ds, bc).map_err(|e| e.to_string())?;
                Ok(if xong { "Dịch offline xong hết.".into() } else { format!("Chưa dịch hết: còn {} câu (đã thử lại, danh sách ở cau_chua_dich.txt trong thư mục dữ liệu game). Bấm dịch lần nữa để thử tiếp.", v.viec.loi) })
            }),
            Some("claude") => self.chay("Đang dịch bằng Claude", move |v, bc| {
                let ds = v.viec.cau_can_dich(v.cac_cau().iter().map(String::as_str));
                let xong = v.viec.chay_claude(ds, gioi_han, bc).map_err(|e| e.to_string())?;
                Ok(format!("{} • tốn {:.3} USD", if xong { "Dịch bằng Claude xong hết" } else { "Chưa dịch hết (bị dừng, chạm giới hạn USD hoặc còn câu hỏng, xem nhật ký) – bấm lại để dịch tiếp" }, v.viec.tien))
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
