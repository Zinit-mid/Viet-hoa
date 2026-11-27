//! Giao diện chính (egui): menu trái QUẢN LÝ / CÔNG CỤ GAME / CÀI ĐẶT.

use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, Color32, RichText};

use crate::cau_hinh::{self, SO_VUNG_TOI_DA};
use crate::dich_man_hinh::{self, Chung, Lenh};
use crate::lop_phu::PhimTat;
use crate::{dich, duong_dan};

/// 6 mục lớn trên thanh trên (theo thiết kế).
#[derive(Clone, Copy, PartialEq)]
enum Muc {
    DichManHinh,
    VietHoa,
    LongTieng,
    PhuDe,
    DoFile,
    CaiDat,
}

const MUC: [(Muc, &str); 6] = [
    (Muc::DichManHinh, "Dịch màn hình"),
    (Muc::VietHoa, "Việt hóa"),
    (Muc::LongTieng, "Lồng tiếng"),
    (Muc::PhuDe, "Phụ đề phim"),
    (Muc::DoFile, "Dò file chữ"),
    (Muc::CaiDat, "Cài đặt"),
];

impl Muc {
    fn so(self) -> usize {
        MUC.iter().position(|m| m.0 == self).unwrap_or(0)
    }
    fn mau(self) -> Color32 {
        match self {
            Muc::VietHoa => crate::kieu::TIM,
            Muc::LongTieng => crate::kieu::CAM,
            Muc::PhuDe => crate::kieu::VANG,
            Muc::DoFile => crate::kieu::XANH_BIEN,
            Muc::DichManHinh => crate::kieu::CYAN,
            Muc::CaiDat => crate::kieu::TIM,
        }
    }
    /// Mục con: (tên, trang, khối cần cuộn tới)
    fn con(self) -> &'static [(&'static str, Trang, Option<&'static str>)] {
        match self {
            Muc::DichManHinh => &[
                ("Vùng dịch", Trang::DichManHinh, Some("VÙNG DỊCH")),
                ("Chế độ dịch", Trang::DichManHinh, Some("MÁY DỊCH")),
                ("Khung chữ dịch", Trang::DichManHinh, Some("KHUNG CHỮ DỊCH")),
                ("Giọng đọc", Trang::DichManHinh, Some("KHUNG CHỮ DỊCH")),
            ],
            Muc::VietHoa => &[
                ("Làm tự động", Trang::TuDong, None),
                ("Thư viện game", Trang::VietHoaGame, None),
                ("Sửa bản dịch", Trang::SuaDich, None),
                ("Bản đồ Crimson Desert", Trang::BanDo, None),
            ],
            Muc::LongTieng => &[
                ("Lồng tiếng game", Trang::LongTieng, None),
                ("Giọng mẫu", Trang::GiongMau, None),
                ("Clone giọng", Trang::CloneGiong, None),
            ],
            Muc::PhuDe => &[
                ("Tạo phụ đề", Trang::PhuDePhim, None),
                ("Thuyết minh", Trang::PhuDePhim, Some("THUYẾT MINH")),
            ],
            Muc::DoFile => &[("Quét và ghi vào game", Trang::DoFile, None)],
            Muc::CaiDat => &[
                ("Cập nhật", Trang::CaiDat, Some("CẬP NHẬT")),
                ("Mã API", Trang::CaiDat, Some("MÃ API")),
                ("Thư mục", Trang::CaiDat, Some("THƯ MỤC")),
                ("Công cụ đi kèm", Trang::CaiDat, Some("CÔNG CỤ ĐI KÈM")),
                ("Giao diện", Trang::CaiDat, Some("GIAO DIỆN")),
                ("AI local", Trang::AiLocal, None),
            ],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Trang {
    DichManHinh,
    PhuDePhim,
    VietHoaGame,
    TuDong,
    SuaDich,
    BanDo,
    AiLocal,
    GiongMau,
    LongTieng,
    CloneGiong,
    DoFile,
    CaiDat,
}

pub struct App {
    chung: Arc<Chung>,
    gui: Sender<Lenh>,
    trang: Trang,
    muc: Muc,
    con: [usize; 6],
    logo: egui::TextureHandle,
    o_khoa: String,
    o_khoa_deepl: String,
    thong_bao: String,
    bao_chuyen: Vec<String>,
    viet_hoa: crate::trang_viet_hoa::TrangVietHoa,
    clone_giong: crate::trang_clone_giong::TrangCloneGiong,
    phu_de_phim: crate::trang_phu_de_phim::TrangPhuDePhim,
    sua_dich: crate::trang_sua_dich::TrangSuaDich,
    ban_do: crate::trang_ban_do::TrangBanDo,
    ai: crate::ai_local::TrangAi,
    tu_dong: crate::trang_tu_dong::TrangTuDong,
    cap_nhat: crate::cap_nhat::CapNhat,
    long_tieng: crate::trang_long_tieng::TrangLongTieng,
    giong_mau: crate::trang_giong_mau::TrangGiongMau,
}

const XANH: Color32 = Color32::from_rgb(64, 160, 255);

impl App {
    pub fn moi(
        cc: &eframe::CreationContext<'_>,
        chung: Arc<Chung>,
        gui: Sender<Lenh>,
        bao_chuyen: Vec<String>,
    ) -> Self {
        crate::kieu::nap_cai_dat();
        crate::kieu::ap_dung(&cc.egui_ctx);
        let app = Self {
            chung,
            gui,
            trang: Trang::DichManHinh,
            muc: Muc::DichManHinh,
            con: [0; 6],
            logo: cc.egui_ctx.load_texture(
                "logo",
                egui::ColorImage::from_rgba_unmultiplied([64, 64], include_bytes!("../assets/icon_64.rgba")),
                egui::TextureOptions::LINEAR,
            ),
            o_khoa: String::new(),
            o_khoa_deepl: String::new(),
            thong_bao: String::new(),
            bao_chuyen,
            viet_hoa: Default::default(),
            clone_giong: Default::default(),
            phu_de_phim: Default::default(),
            sua_dich: Default::default(),
            ban_do: Default::default(),
            ai: Default::default(),
            tu_dong: Default::default(),
            cap_nhat: {
                crate::cap_nhat::don_dep();
                let c = crate::cap_nhat::CapNhat::default();
                c.kiem_tra_nen(true);
                c
            },
            long_tieng: Default::default(),
            giong_mau: Default::default(),
        };
        app.giong_mau.dang_ky_tt();
        app.viet_hoa.dang_ky_tt();
        app.clone_giong.dang_ky_tt();
        app.phu_de_phim.dang_ky_tt();
        app.tu_dong.dang_ky_tt();
        app.long_tieng.dang_ky_tt();
        app.ai.dang_ky_tt();
        app.cap_nhat.dang_ky_tt();
        app
    }
}

impl App {
    fn chon_con(&mut self, ctx: &egui::Context, muc: Muc, i: usize) {
        self.muc = muc;
        self.con[muc.so()] = i;
        let (_, t, anchor) = muc.con()[i];
        self.trang = t;
        if let Some(a) = anchor {
            crate::kieu::yeu_cau_cuon(ctx, a);
        }
    }

    fn doi_muc(&mut self, ctx: &egui::Context, muc: Muc) {
        let i = self.con[muc.so()].min(muc.con().len() - 1);
        self.chon_con(ctx, muc, i);
    }

    /// Thanh trên: logo, 6 mục lớn, nút "Dịch cả màn hình".
    fn thanh_tren(&mut self, ctx: &egui::Context) {
        use crate::kieu as k;
        egui::TopBottomPanel::top("thanh_tren")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 20, top: 14, bottom: 6 }))
            .show(ctx, |ui| {
                k::kinh_khung().inner_margin(egui::Margin::symmetric(24, 12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // logo
                        ui.add(egui::Image::new((self.logo.id(), egui::vec2(52.0, 52.0))));
                        let rong = ui.ctx().screen_rect().width();
                        if rong >= 1250.0 {
                            ui.label(k::dam("VIỆT HÓA").size(28.0).extra_letter_spacing(2.0));
                        }
                        ui.add_space(if rong >= 1250.0 { 24.0 } else { 6.0 });
                        let mut doi = None;
                        for (m, ten) in MUC {
                            let chon = self.muc == m;
                            let ro = ui.add(
                                egui::Label::new(if chon { k::dam(ten).size(18.0) } else { egui::RichText::new(ten).size(18.0).color(k::CHU_PHU) })
                                    .sense(egui::Sense::click()),
                            );
                            let ro = ro.on_hover_cursor(egui::CursorIcon::PointingHand);
                            if chon {
                                let y = ro.rect.bottom() + 6.0;
                                ui.painter().line_segment([egui::pos2(ro.rect.left(), y), egui::pos2(ro.rect.right(), y)], egui::Stroke::new(3.0, k::TIM));
                            }
                            if ro.clicked() {
                                doi = Some(m);
                            }
                            ui.add_space(if ui.ctx().screen_rect().width() >= 1250.0 { 14.0 } else { 4.0 });
                        }
                        if let Some(m) = doi {
                            self.doi_muc(ctx, m);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tm = self.chung.toan_man.load(Ordering::SeqCst);
                            let nhan = if tm { "● Đang dịch · bấm để dừng" } else { "Dịch cả màn hình (F12)" };
                            if k::pill(ui, nhan, if tm { k::OK } else { k::XANH }, true).clicked() {
                                let _ = self.gui.send(Lenh::Phim(PhimTat::ToanMan));
                            }
                            if let Some(b) = self.cap_nhat.0.lock().unwrap().ban.as_ref() {
                                if ui.link(egui::RichText::new(format!("⬆ Có bản mới {}", b.ma)).color(k::OK)).clicked() {
                                    self.muc = Muc::CaiDat;
                                    self.trang = Trang::CaiDat;
                                    k::yeu_cau_cuon(ctx, "CẬP NHẬT");
                                }
                            }
                        });
                    });
                });
            });
    }

    /// Cột trái: mục con của mục lớn đang chọn.
    fn cot_trai(&mut self, ctx: &egui::Context) {
        use crate::kieu as k;
        egui::SidePanel::left("muc_con")
            .exact_width(276.0)
            .resizable(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 20, right: 8, top: 8, bottom: 8 }))
            .show(ctx, |ui| {
                k::kinh_khung().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(egui::RichText::new("MỤC CON").size(14.0).color(k::CHU_MO));
                    ui.add_space(6.0);
                    let muc = self.muc;
                    let cur = self.con[muc.so()];
                    let mut doi = None;
                    for (i, (ten, _, _)) in muc.con().iter().enumerate() {
                        let (rect, ph) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 48.0), egui::Sense::click());
                        let chon = i == cur;
                        let p = ui.painter();
                        if chon {
                            p.rect_filled(rect, 16.0, crate::kieu::rgba(185, 138, 255, 48));
                            p.rect_stroke(rect, 16.0, egui::Stroke::new(1.4, crate::kieu::rgba(185, 138, 255, 190)), egui::StrokeKind::Inside);
                        } else if ph.hovered() {
                            p.rect_filled(rect, 16.0, crate::kieu::rgba(190, 150, 255, 26));
                        }
                        p.text(
                            egui::pos2(rect.left() + 18.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            ten,
                            egui::FontId::new(17.0, if chon { k::dam_ho() } else { egui::FontFamily::Proportional }),
                            if chon { k::CHU } else { k::CHU_PHU },
                        );
                        if ph.clicked() {
                            doi = Some(i);
                        }
                    }
                    if let Some(i) = doi {
                        self.chon_con(ctx, muc, i);
                    }
                });
            });
    }

    /// Cột phải: Việc đang chạy + Kho giọng.
    fn cot_phai(&mut self, ctx: &egui::Context) {
        use crate::kieu as k;
        egui::SidePanel::right("cot_phai")
            .exact_width(336.0)
            .resizable(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 8, right: 20, top: 8, bottom: 8 }))
            .show(ctx, |ui| {
                k::kinh_khung().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(k::dam("Việc đang chạy").size(20.0));
                    ui.add_space(6.0);
                    let dang = crate::tien_trinh::dang_chay();
                    let xong = crate::tien_trinh::vua_xong();
                    if dang.is_empty() && xong.is_empty() {
                        ui.label(egui::RichText::new("Không có việc nào đang chạy.").size(14.0).color(k::CHU_MO));
                    }
                    for (khoa, a, giay) in &dang {
                        ui.horizontal(|ui| {
                            ui.label(k::dam(if a.ten.is_empty() { "Đang làm việc" } else { &a.ten }).size(16.0));
                            ui.label(egui::RichText::new(crate::tien_trinh::dung_mm(*giay)).size(14.0).color(k::CHU_MO));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("Dừng").clicked() {
                                    crate::tien_trinh::dung_viec(khoa);
                                }
                            });
                        });
                        k::thanh_nho(ui, a.tien, k::XANH_SANG);
                        if !a.thong_bao.is_empty() {
                            ui.add(egui::Label::new(egui::RichText::new(&a.thong_bao).size(14.0).color(k::CHU_PHU)).truncate());
                        }
                        ui.add_space(6.0);
                    }
                    let mut tat = Vec::new();
                    for (khoa, tb, loi) in &xong {
                        ui.horizontal(|ui| {
                            let m = if *loi { k::LOI_CHU } else { k::OK };
                            ui.label(egui::RichText::new(if *loi { "✘" } else { "✔" }).color(m));
                            ui.add(egui::Label::new(egui::RichText::new(tb).size(14.0).color(m)).truncate());
                            if ui.small_button("Ẩn").clicked() {
                                tat.push(*khoa);
                            }
                        });
                    }
                    for t in tat {
                        crate::tien_trinh::tat_xong(t);
                    }
                });
                ui.add_space(14.0);
                k::kinh_khung().show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(k::dam("Kho giọng").size(20.0));
                    ui.add_space(6.0);
                    let ds = crate::viet_hoa::clone_giong::liet_ke_chinh();
                    if ds.is_empty() {
                        ui.label(egui::RichText::new("Chưa có giọng nào.").size(14.0).color(k::CHU_MO));
                    }
                    ui.horizontal_wrapped(|ui| {
                        for f in ds.iter().take(3) {
                            ui.label(egui::RichText::new(f.trim_end_matches(".wav")).family(egui::FontFamily::Monospace).size(14.0));
                            ui.add_space(6.0);
                        }
                    });
                    if ds.len() > 3 {
                        ui.label(egui::RichText::new(format!("… và {} giọng khác", ds.len() - 3)).size(14.0).color(k::CHU_MO));
                    }
                    ui.add_space(8.0);
                    if k::pill_phu(ui, "+ Clone giọng mới", true).clicked() {
                        self.muc = Muc::LongTieng;
                        self.chon_con(ctx, Muc::LongTieng, 2);
                    }
                });
            });
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        crate::kieu::mau_nen()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.chung.can_luu.swap(false, Ordering::SeqCst) {
            self.chung.ch().luu();
        }
        ctx.request_repaint_after(Duration::from_millis(300));
        crate::tien_trinh::cap_nhat();
        crate::kieu::dat_mau_vien(crate::kieu::TIM);
        crate::kieu::nen_anh_sang(ctx);

        egui::TopBottomPanel::bottom("trang_thai")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(24, 6)))
            .show(ctx, |ui| {
                let dang = self.chung.dang_dich.load(Ordering::SeqCst) || self.chung.toan_man.load(Ordering::SeqCst);
                let (chu, mau) = if dang { ("● Đang chạy dịch màn hình", crate::kieu::OK) } else { ("○ Dịch màn hình đang dừng", crate::kieu::CHU_MO) };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(chu).size(14.0).color(mau));
                    if !self.thong_bao.is_empty() {
                        ui.separator();
                        ui.label(RichText::new(&self.thong_bao).size(14.0));
                    }
                });
            });

        let rong = ctx.screen_rect().width();
        let hien_cot_phai = rong >= 1280.0;
        if !hien_cot_phai {
            crate::tien_trinh::ve_dai_dau(ctx);
        }
        self.thanh_tren(ctx);
        self.cot_trai(ctx);
        if hien_cot_phai {
            self.cot_phai(ctx);
        }

        egui::CentralPanel::default().frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 4 })).show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.trang {
                Trang::DichManHinh => self.trang_dich(ui),
                Trang::CaiDat => self.trang_cai_dat(ui),
                Trang::PhuDePhim => self.phu_de_phim.ve(ui),
                Trang::VietHoaGame => self.viet_hoa.ve(ui),
                Trang::DoFile => self.viet_hoa.ve(ui),
                Trang::TuDong => self.tu_dong.ve(ui),
                Trang::SuaDich => self.sua_dich.ve(ui),
                Trang::BanDo => self.ban_do.ve(ui),
                Trang::AiLocal => self.ai.ve(ui),
                Trang::GiongMau => self.giong_mau.ve(ui),
                Trang::LongTieng => self.long_tieng.ve(ui),
                Trang::CloneGiong => self.clone_giong.ve(ui),
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.chung.ch().luu();
        dich::luu_bo_nho();
        dich::offline::tat();
    }
}

fn sap_co(ui: &mut egui::Ui, tieu_de: &str, mo_ta: &str) {
    ui.heading(tieu_de);
    ui.add_space(6.0);
    ui.label(mo_ta);
    ui.add_space(10.0);
    ui.label(
        RichText::new("Phần này đang được viết lại bằng Rust – sẽ có ở bản sau.")
            .color(Color32::YELLOW),
    );
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    crate::kieu::khoi(ui, tieu_de, noi);
}

impl App {
    fn trang_dich(&mut self, ui: &mut egui::Ui) {
        ui.heading("Dịch màn hình");
        ui.label("Đọc chữ tiếng Anh trong các vùng bạn khoanh rồi hiện bản dịch tiếng Việt ở khung dịch.");
        ui.add_space(8.0);
        let chung = self.chung.clone();
        let mut doi = false;

        the(ui, "ĐIỀU KHIỂN", |ui| {
            ui.horizontal_wrapped(|ui| {
                let dang = chung.dang_dich.load(Ordering::SeqCst);
                let nut = if dang {
                    "■  Dừng dịch tự động (F8)"
                } else {
                    "▶  Bật dịch tự động (F8)"
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(nut).strong())
                            .min_size(egui::vec2(230.0, 36.0)),
                    )
                    .clicked()
                {
                    let _ = self.gui.send(Lenh::Phim(PhimTat::BatTatDich));
                }
                let tm = chung.toan_man.load(Ordering::SeqCst);
                let nut2 = if tm {
                    "■  Tắt dịch cả màn hình (F12)"
                } else {
                    "⛶  Dịch cả màn hình (F12)"
                };
                if ui
                    .add(egui::Button::new(nut2).min_size(egui::vec2(230.0, 36.0)))
                    .clicked()
                {
                    let _ = self.gui.send(Lenh::Phim(PhimTat::ToanMan));
                }
                let chinh = crate::khung_dich::dang_chinh();
                let nut3 = if chinh {
                    "✔  Đang chỉnh khung – bấm để khóa (F11)"
                } else {
                    "Chỉnh khung bằng chuột (F11)"
                };
                if ui
                    .add(
                        egui::Button::new(nut3)
                            .selected(chinh)
                            .min_size(egui::vec2(230.0, 36.0)),
                    )
                    .clicked()
                {
                    let _ = self.gui.send(Lenh::Phim(PhimTat::ChinhKhung));
                }
            });
            ui.label(
                RichText::new(
                    "F6 / F7 chọn lại vùng Hội thoại / Nhiệm vụ • F9 dịch ngay • F10 ẩn/hiện bản dịch • \
                     Ctrl+mũi tên dời khung Hội thoại • Ctrl+Shift+mũi tên dời khung Nhiệm vụ • \
                     Ctrl+Shift+V giọng đọc • Ctrl+Shift+C bật/tắt Claude",
                )
                .small()
                .weak(),
            );
        });

        // Các vùng
        let mut chon: Option<usize> = None;
        the(
            ui,
            &format!("VÙNG DỊCH (tối đa {SO_VUNG_TOI_DA})"),
            |ui| {
                let mut ch = chung.ch();
                let mut xoa = None;
                for (i, v) in ch.cac_vung.iter_mut().enumerate() {
                    egui::Frame::NONE.inner_margin(4.0).show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{}.", i + 1)).strong());
                            doi |= ui
                                .add(egui::TextEdit::singleline(&mut v.ten).desired_width(110.0))
                                .changed();
                            egui::ComboBox::from_id_salt(("loai", &v.id))
                                .selected_text(if v.loai == "co_dinh" {
                                    "Cố định (nhiệm vụ, menu)"
                                } else {
                                    "Hội thoại / phụ đề"
                                })
                                .show_ui(ui, |ui| {
                                    doi |= ui
                                        .selectable_value(
                                            &mut v.loai,
                                            "phu_de".into(),
                                            "Hội thoại / phụ đề – dịch nhanh",
                                        )
                                        .changed();
                                    doi |= ui
                                        .selectable_value(
                                            &mut v.loai,
                                            "co_dinh".into(),
                                            "Cố định – nhiệm vụ, menu, vật phẩm",
                                        )
                                        .changed();
                                });
                            let ten_nut = if v.vung.is_some() {
                                "Chọn lại vùng chữ"
                            } else {
                                "➤ Chọn vùng chữ"
                            };
                            if ui.button(ten_nut).clicked() {
                                chon = Some(i);
                            }
                            doi |= crate::kieu::cong_tac(ui, &mut v.loc_trang, "Lọc chữ trắng").changed();
                            ui.label("Cỡ chữ");
                            doi |= ui
                                .add(egui::DragValue::new(&mut v.co_chu).range(10..=48))
                                .changed();
                            if i >= 2 && ui.button("Xóa").clicked() {
                                xoa = Some(i);
                            }
                        });
                        match v.vung {
                            Some(r) => ui.label(
                                RichText::new(format!(
                                    "Vùng chữ {}×{} tại ({}, {})",
                                    r[2], r[3], r[0], r[1]
                                ))
                                .small()
                                .weak(),
                            ),
                            None => ui.label(
                                RichText::new("Chưa chọn vùng chữ")
                                    .small()
                                    .color(Color32::YELLOW),
                            ),
                        };
                    });
                }
                if let Some(i) = xoa {
                    ch.cac_vung.remove(i);
                    doi = true;
                }
                if ch.cac_vung.len() < SO_VUNG_TOI_DA && ui.button("+ Thêm vùng").clicked() {
                    chon = Some(ch.cac_vung.len());
                }
            },
        );
        if let Some(i) = chon {
            dich_man_hinh::chon_vung(&chung, &self.gui, i);
        }

        {
            let mut ch = chung.ch();
            the(ui, "MÁY DỊCH", |ui| {
                let ten = |m: &str| match m {
                    "offline" => "Offline (nhanh, không cần mạng)",
                    "offline_claude" => "Offline + Claude dịch ngầm để lưu (khuyên dùng)",
                    "claude" => "Claude (hay nhất, tốn phí)",
                    _ => "Google (miễn phí, cần mạng)",
                };
                egui::ComboBox::from_id_salt("che_do")
                    .width(360.0)
                    .selected_text(ten(&ch.che_do_hoi_thoai))
                    .show_ui(ui, |ui| {
                        for m in ["offline_claude", "offline", "claude", "google"] {
                            doi |= ui
                                .selectable_value(&mut ch.che_do_hoi_thoai, m.to_string(), ten(m))
                                .changed();
                        }
                    });
                doi |= ui
                    .checkbox(
                        &mut ch.thay_bang_claude,
                        "Hội thoại: đổi sang bản Claude khi dịch xong",
                    )
                    .changed();
                if !dich::offline::co_the_dung() {
                    ui.label(RichText::new("Chưa có Python riêng / mô hình offline > tạm dùng Google. Vào Cài đặt > Tải công cụ.").color(Color32::YELLOW));
                }
                if let Some(ly) = dich::claude::ly_do_khong_dung() {
                    ui.label(RichText::new(ly).color(Color32::YELLOW));
                } else if !dich::claude::BAT.load(Ordering::Relaxed) {
                    ui.label(RichText::new("Claude đang TẮT cho dịch màn hình (bấm Ctrl+Shift+C để bật). Việt hóa game và phụ đề phim vẫn dùng Claude bình thường.").weak());
                }
            });
            the(ui, "KHUNG CHỮ DỊCH", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Kiểu hiện:");
                    doi |= ui
                        .selectable_value(
                            &mut ch.kieu_hien,
                            "phu_de".into(),
                            "Phụ đề (chữ viền, nền trong suốt)",
                        )
                        .changed();
                    doi |= ui
                        .selectable_value(&mut ch.kieu_hien, "khung".into(), "Khung nền tối")
                        .changed();
                });
                doi |= ui
                    .checkbox(
                        &mut ch.che_chu_goc,
                        "Che chữ gốc (ô cùng màu nền đè lên chữ tiếng Anh)",
                    )
                    .changed();
                ui.horizontal(|ui| {
                    if ch.kieu_hien == "khung" {
                        let mut v = ch.do_mo as f64;
                        if crate::kieu::thanh_truot(ui, &mut v, 0.2..=1.0, "Độ đậm", 2).changed() {
                            ch.do_mo = v as _;
                            doi = true;
                        }
                    } else {
                        let mut v = ch.do_dam_chu as f64;
                        if crate::kieu::thanh_truot(ui, &mut v, 0.2..=1.0, "Độ đậm chữ", 2).changed() {
                            ch.do_dam_chu = v as _;
                            doi = true;
                        }
                        let mut w = ch.vien_chu as f64;
                        if crate::kieu::thanh_truot(ui, &mut w, 0.0..=5.0, "Viền chữ", 0).changed() {
                            ch.vien_chu = w.round() as _;
                            doi = true;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    doi |= ui
                        .checkbox(&mut ch.giong_doc, "Đọc hội thoại thành tiếng")
                        .changed();
                    egui::ComboBox::from_id_salt("giong")
                        .selected_text(if ch.giong.contains("HoaiMy") {
                            "Nữ – Hoài My"
                        } else {
                            "Nam – Nam Minh"
                        })
                        .show_ui(ui, |ui| {
                            doi |= ui
                                .selectable_value(
                                    &mut ch.giong,
                                    "vi-VN-NamMinhNeural".into(),
                                    "Nam – Nam Minh",
                                )
                                .changed();
                            doi |= ui
                                .selectable_value(
                                    &mut ch.giong,
                                    "vi-VN-HoaiMyNeural".into(),
                                    "Nữ – Hoài My",
                                )
                                .changed();
                        });
                });
            });
        }
        if doi {
            chung.ch().luu();
        }

        let tt = chung.tt();
        the(ui, "TÌNH TRẠNG", |ui| {
            ui.label(format!("Cách chụp: {}   •   OCR: {}", tt.may_chup, tt.ocr));
            if chung.an_ban_dich.load(Ordering::SeqCst) {
                ui.label(RichText::new("Bản dịch đang ẨN (F10 để hiện).").color(Color32::YELLOW));
            }
            if chung.toan_man.load(Ordering::SeqCst) {
                ui.label(format!("Cả màn hình: {} đoạn chữ", tt.so_doan_toan_man));
            }
            if !tt.cuoi_en.is_empty() {
                ui.label(RichText::new(format!("EN: {}", tt.cuoi_en)).weak());
                ui.label(format!(
                    "VI ({}, {} ms): {}",
                    tt.cuoi_may, tt.cuoi_ms, tt.cuoi_vi
                ));
            }
            if !tt.loi.is_empty() {
                ui.label(RichText::new(&tt.loi).color(Color32::LIGHT_RED));
            }
        });
    }

    fn ve_cap_nhat(&mut self, ui: &mut egui::Ui) {
        let cn = self.cap_nhat.clone();
        let (kiem, tai, tien, tb, loi, ban, da_thay) = {
            let t = cn.0.lock().unwrap();
            (t.dang_kiem, t.dang_tai, t.tien, t.tb.clone(), t.loi, t.ban.clone(), t.da_thay)
        };
        if kiem || tai {
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        }
        the(ui, "CẬP NHẬT", |ui| {
            ui.label(format!("Phiên bản đang chạy: {}  •  nguồn: github.com/{}", crate::cap_nhat::PHIEN_BAN, crate::cap_nhat::KHO));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!kiem && !tai && !da_thay, egui::Button::new("Kiểm tra bản mới")).clicked() {
                    cn.kiem_tra_nen(false);
                }
                if let Some(b) = &ban {
                    if ui
                        .add_enabled(!tai && !da_thay, egui::Button::new(RichText::new(format!("Cập nhật lên {}", b.ma)).strong()))
                        .clicked()
                    {
                        cn.cap_nhat_nen();
                    }
                }
                if da_thay && ui.button(RichText::new("Khởi động lại").strong()).clicked() {
                    crate::cap_nhat::mo_lai();
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if tai {
                ui.add(egui::ProgressBar::new(tien).show_percentage());
            }
            if !tb.is_empty() {
                ui.label(if loi { RichText::new(tb).color(Color32::LIGHT_RED) } else { RichText::new(tb) });
            }
            if let Some(b) = &ban {
                if !b.ghi_chu.trim().is_empty() {
                    ui.label(RichText::new("Có gì mới:").small().weak());
                    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| ui.label(RichText::new(&b.ghi_chu).small()));
                }
            }
        });
    }

    fn trang_cai_dat(&mut self, ui: &mut egui::Ui) {
        ui.heading("Cài đặt");
        ui.add_space(8.0);
        self.ve_cap_nhat(ui);
        the(ui, "MÃ API", |ui| {
            for (ten, file, o) in [
                ("Claude", "khoa_api.txt", &mut self.o_khoa),
                ("DeepL", "khoa_deepl.txt", &mut self.o_khoa_deepl),
            ] {
                ui.horizontal(|ui| {
                    let co = cau_hinh::doc_khoa(file).is_some();
                    ui.label(format!("{ten}:"));
                    ui.label(if co {
                        RichText::new("✔ đã có").color(Color32::LIGHT_GREEN)
                    } else {
                        RichText::new("chưa có").color(Color32::YELLOW)
                    });
                    ui.add(
                        egui::TextEdit::singleline(o)
                            .password(true)
                            .hint_text("dán mã mới vào đây")
                            .desired_width(260.0),
                    );
                    if ui.button("Lưu mã").clicked() && !o.trim().is_empty() {
                        self.thong_bao = match cau_hinh::luu_khoa(file, o) {
                            Ok(()) => format!("Đã lưu mã {ten} vào thư mục dữ liệu"),
                            Err(e) => format!("Lỗi lưu mã: {e}"),
                        };
                        o.clear();
                        dich::claude::KHOA_HONG.store(false, Ordering::Relaxed);
                    }
                });
            }
            ui.label(
                RichText::new(
                    "Mã chỉ lưu trong du_lieu trên máy này, không gửi đi đâu ngoài máy chủ dịch.",
                )
                .small()
                .weak(),
            );
        });

        the(ui, "GIAO DIỆN", |ui| {
            let mut b = crate::kieu::anh_sang_bat();
            if crate::kieu::cong_tac(ui, &mut b, "Hiệu ứng ánh sáng nền").changed() {
                crate::kieu::dat_anh_sang(b);
            }
            ui.label(RichText::new("Tắt nếu máy yếu hoặc chạy cạnh game nặng. Giao diện chỉ có bản tối.").size(14.0).color(crate::kieu::CHU_MO));
        });
        the(ui, "THƯ MỤC", |ui| {
            ui.label(format!("App:      {}", duong_dan::goc().display()));
            ui.label(format!("Dữ liệu:  {}", duong_dan::du_lieu().display()));
            ui.label(format!("Công cụ:  {}", duong_dan::cong_cu().display()));
            if ui.button("Tạo lối tắt trên Desktop").clicked() {
                self.thong_bao = match duong_dan::tao_loi_tat_desktop() {
                    Ok(p) => format!("Đã tạo lối tắt: {}", p.display()),
                    Err(e) => format!("Không tạo được lối tắt: {e}"),
                };
            }
            ui.label(
                RichText::new("Đừng chép file VietHoa.exe ra chỗ khác – hãy dùng lối tắt.")
                    .small()
                    .weak(),
            );
            if ui.button("Mở thư mục dữ liệu").clicked() {
                let _ = std::fs::create_dir_all(duong_dan::du_lieu());
                let _ = std::process::Command::new("explorer")
                    .arg(duong_dan::du_lieu())
                    .spawn();
            }
            for b in &self.bao_chuyen {
                ui.label(RichText::new(b).small().weak());
            }
        });

        the(ui, "CÔNG CỤ ĐI KÈM", |ui| {
            let cc = duong_dan::cong_cu();
            let ds: [(&str, std::path::PathBuf); 6] = [
                ("Python riêng (AI)", cc.join("python/python.exe")),
                ("ffmpeg", cc.join("ffmpeg/bin/ffmpeg.exe")),
                ("vgmstream-cli", cc.join("vgmstream/vgmstream-cli.exe")),
                ("VLC", cc.join("vlc/vlc.exe")),
                (
                    "Font Be Vietnam Pro",
                    duong_dan::goc().join("assets/fonts/BeVietnamPro-Bold.ttf"),
                ),
                (
                    "Mô hình dịch offline",
                    duong_dan::mo_hinh_offline()
                        .unwrap_or_default()
                        .join("model.bin"),
                ),
            ];
            for (ten, p) in ds {
                let co = p.is_file();
                ui.label(if co {
                    RichText::new(format!("✔  {ten}")).color(Color32::LIGHT_GREEN)
                } else {
                    RichText::new(format!("✘  {ten}")).color(Color32::LIGHT_RED)
                });
            }
            if ui.button("Tải công cụ còn thiếu").clicked() {
                let script = duong_dan::goc().join("scripts/tai_cong_cu.ps1");
                let _ = std::process::Command::new("powershell")
                    .args(["-NoExit", "-ExecutionPolicy", "Bypass", "-File"])
                    .arg(script)
                    .spawn();
            }
            ui.label(RichText::new("Chưa tự tải được (cần tài khoản / bản quyền): Wwise 2025 (Audiokinetic), DMM. Gói OCR tiếng Anh cài trong Cài đặt Windows > Ngôn ngữ.").small().weak());
        });
    }
}
