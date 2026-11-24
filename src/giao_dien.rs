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

#[derive(Clone, Copy, PartialEq)]
enum Trang {
    DichManHinh,
    PhuDePhim,
    VietHoaGame,
    LongTieng,
    CloneGiong,
    CaiDat,
}

pub struct App {
    chung: Arc<Chung>,
    gui: Sender<Lenh>,
    trang: Trang,
    o_khoa: String,
    o_khoa_deepl: String,
    thong_bao: String,
    bao_chuyen: Vec<String>,
    viet_hoa: crate::trang_viet_hoa::TrangVietHoa,
}

const XANH: Color32 = Color32::from_rgb(64, 160, 255);

impl App {
    pub fn moi(
        cc: &eframe::CreationContext<'_>,
        chung: Arc<Chung>,
        gui: Sender<Lenh>,
        bao_chuyen: Vec<String>,
    ) -> Self {
        nap_font_egui(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        Self {
            chung,
            gui,
            trang: Trang::DichManHinh,
            o_khoa: String::new(),
            o_khoa_deepl: String::new(),
            thong_bao: String::new(),
            bao_chuyen,
            viet_hoa: Default::default(),
        }
    }
}

fn nap_font_egui(ctx: &egui::Context) {
    let thu = [
        duong_dan::goc().join("assets/fonts/BeVietnamPro-Regular.ttf"),
        std::path::PathBuf::from(r"C:\Windows\Fonts\segoeui.ttf"),
    ];
    if let Some(b) = thu.iter().find_map(|p| std::fs::read(p).ok()) {
        let mut f = egui::FontDefinitions::default();
        f.font_data
            .insert("viet".into(), Arc::new(egui::FontData::from_owned(b)));
        for ho in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            f.families.entry(ho).or_default().insert(0, "viet".into());
        }
        ctx.set_fonts(f);
    }
    ctx.style_mut(|s| {
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(15.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
    });
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.chung.can_luu.swap(false, Ordering::SeqCst) {
            self.chung.ch().luu();
        }
        ctx.request_repaint_after(Duration::from_millis(300));

        egui::TopBottomPanel::bottom("trang_thai").show(ctx, |ui| {
            let dang = self.chung.dang_dich.load(Ordering::SeqCst)
                || self.chung.toan_man.load(Ordering::SeqCst);
            let (chu, mau) = if dang {
                ("Đang chạy dịch màn hình", Color32::LIGHT_GREEN)
            } else {
                ("Đã dừng", Color32::LIGHT_RED)
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(chu).color(mau));
                if !self.thong_bao.is_empty() {
                    ui.separator();
                    ui.label(&self.thong_bao);
                }
            });
        });

        egui::SidePanel::left("menu")
            .exact_width(230.0)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                ui.heading(RichText::new("Việt Hóa").color(XANH).strong());
                ui.label(
                    RichText::new(concat!("Phiên bản ", env!("CARGO_PKG_VERSION")))
                        .small()
                        .weak(),
                );
                ui.add_space(10.0);
                let muc = |ui: &mut egui::Ui, trang: &mut Trang, t: Trang, chu: &str| {
                    if ui
                        .add_sized([210.0, 32.0], egui::SelectableLabel::new(*trang == t, chu))
                        .clicked()
                    {
                        *trang = t;
                    }
                };
                ui.label(RichText::new("QUẢN LÝ").small().weak());
                muc(ui, &mut self.trang, Trang::DichManHinh, "🖵  Dịch màn hình");
                muc(ui, &mut self.trang, Trang::PhuDePhim, "🎬  Phụ đề phim");
                ui.add_space(6.0);
                ui.label(RichText::new("CÔNG CỤ GAME").small().weak());
                muc(ui, &mut self.trang, Trang::VietHoaGame, "📝  Việt hóa game");
                muc(ui, &mut self.trang, Trang::LongTieng, "🔊  Lồng tiếng");
                muc(ui, &mut self.trang, Trang::CloneGiong, "🎤  Clone giọng");
                ui.add_space(6.0);
                ui.label(RichText::new("CÀI ĐẶT").small().weak());
                muc(ui, &mut self.trang, Trang::CaiDat, "⚙  Cài đặt");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.trang {
                Trang::DichManHinh => self.trang_dich(ui),
                Trang::CaiDat => self.trang_cai_dat(ui),
                Trang::PhuDePhim => sap_co(ui, "Phụ đề phim", "Link YouTube / file phim > nghe lời thoại (Whisper large-v3-turbo, GPU) > dịch > file .srt tiếng Việt cạnh phim > mở VLC kèm phụ đề."),
                Trang::VietHoaGame => self.viet_hoa.ve(ui),
                Trang::LongTieng => sap_co(ui, "Lồng tiếng", "Ghép file giọng - câu phụ đề > nhóm người nói > giọng mẫu > tự sửa giới tính (F0) > phiên âm tên riêng > VieNeu > hậu kỳ > mã hóa lại (Wwise .wem / FSB4 trong .mul)."),
                Trang::CloneGiong => sap_co(ui, "Clone giọng", "Chọn file ghi âm + giây bắt đầu–kết thúc > cắt, bỏ lặng, mono 24 kHz, lọc 70 Hz, chuẩn hóa độ to > nghe thử bằng VieNeu > lưu vào kho giọng > gán cho nhân vật từng game."),
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
    egui::Frame::group(ui.style())
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(tieu_de).strong().color(XANH));
            ui.add_space(4.0);
            noi(ui);
        });
    ui.add_space(8.0);
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
                            doi |= ui.checkbox(&mut v.loc_trang, "Lọc chữ trắng").changed();
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
                if !dich::claude::co_khoa() {
                    ui.label(
                        RichText::new("Claude đang tắt hoặc chưa có mã API (Cài đặt > Mã API).")
                            .weak(),
                    );
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
                    ui.label("Độ đậm:");
                    if ch.kieu_hien == "khung" {
                        doi |= ui
                            .add(egui::Slider::new(&mut ch.do_mo, 0.2..=1.0))
                            .changed();
                    } else {
                        doi |= ui
                            .add(egui::Slider::new(&mut ch.do_dam_chu, 0.2..=1.0))
                            .changed();
                        ui.label("Viền chữ:");
                        doi |= ui.add(egui::Slider::new(&mut ch.vien_chu, 0..=5)).changed();
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

    fn trang_cai_dat(&mut self, ui: &mut egui::Ui) {
        ui.heading("Cài đặt");
        ui.add_space(8.0);
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
