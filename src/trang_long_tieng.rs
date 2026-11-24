//! Trang Lồng tiếng (game bất kỳ có file thoại rời wav / ogg / mp3 / flac / opus):
//! quét âm thanh → ghép chữ (+ dịch câu thiếu) → chọn giọng từng người → lồng tiếng → cài / gỡ.
//! Crimson Desert và Tomb Raider có lồng tiếng riêng trong mục Làm tự động / Việt hóa game.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::viet_hoa::do_file::ViecFile;
use crate::viet_hoa::tim_game::{self, Game};
use crate::viet_hoa::tomb_raider::thu_muc_lam_viec;
use crate::viet_hoa::viec_dich::BaoCao;
use crate::viet_hoa::{clone_giong, long_tieng_chung as lt};
use crate::{dich, duong_dan};

#[derive(Default)]
struct Tt {
    dang_ban: bool,
    thong_bao: String,
    loi: bool,
    tien: f32,
    clips: Vec<lt::Clip>,
}

struct Bao(Arc<Mutex<Tt>>);

impl BaoCao for Bao {
    fn bao(&self, s: &str) {
        self.0.lock().unwrap().thong_bao = s.to_string();
    }
    fn tien_do(&self, xong: usize, tong: usize, _usd: f64) {
        self.0.lock().unwrap().tien = if tong > 0 { xong as f32 / tong as f32 } else { 0.0 };
    }
}

pub struct TrangLongTieng {
    games: Option<Vec<Game>>,
    dang_tim: Arc<Mutex<Option<Vec<Game>>>>,
    da_goi: bool,
    chon: Option<usize>,
    chon_cu: Option<usize>,
    tt: Arc<Mutex<Tt>>,
    dung: Arc<AtomicBool>,
    gan: HashMap<String, String>,
    kho: Vec<String>,
    kho_so: usize,
    them: crate::them_giong::ThemGiong,
}

impl Default for TrangLongTieng {
    fn default() -> Self {
        Self {
            games: None,
            dang_tim: Arc::new(Mutex::new(None)),
            da_goi: false,
            chon: None,
            chon_cu: None,
            tt: Arc::new(Mutex::new(Tt::default())),
            dung: Arc::new(AtomicBool::new(false)),
            gan: HashMap::new(),
            kho: clone_giong::liet_ke(),
            kho_so: 0,
            them: Default::default(),
        }
    }
}

fn lv_cua(g: &Game) -> PathBuf {
    lt::thu_muc_lv(&thu_muc_lam_viec(&g.thu_muc), &g.ten)
}

fn luu(lv: &std::path::Path, clips: &[lt::Clip]) {
    let _ = std::fs::create_dir_all(lv);
    if let Ok(b) = serde_json::to_vec(clips) {
        let _ = duong_dan::ghi_file(&lv.join("clips.json"), &b);
    }
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(tieu_de).color(Color32::from_rgb(64, 160, 255)).strong());
        noi(ui);
    });
    ui.add_space(6.0);
}

impl TrangLongTieng {
    pub fn dang_ky_tt(&self) {
        let t = self.tt.clone();
        crate::tien_trinh::dang_ky("long_tieng", crate::tien_trinh::co_dinh(self.dung.clone()), move || {
            let g = t.lock().unwrap();
            crate::tien_trinh::Anh {
                dang_ban: g.dang_ban,
                ten: "Lồng tiếng".into(),
                thong_bao: g.thong_bao.clone(),
                tien: (g.tien > 0.0).then_some(g.tien),
                loi: g.loi,
            }
        });
    }

    /// Chạy việc nền; `viec` trả thông báo kết quả.
    fn nen(&self, ten: &str, viec: impl FnOnce(&Arc<Mutex<Tt>>, &Arc<AtomicBool>) -> anyhow::Result<String> + Send + 'static) {
        {
            let mut t = self.tt.lock().unwrap();
            if t.dang_ban {
                return;
            }
            t.dang_ban = true;
            t.loi = false;
            t.tien = 0.0;
            t.thong_bao = format!("{ten}...");
        }
        self.dung.store(false, Ordering::SeqCst);
        let (tt, dung) = (self.tt.clone(), self.dung.clone());
        std::thread::spawn(move || {
            let _thuc = duong_dan::GiuMayThuc::moi();
            let kq = viec(&tt, &dung);
            let mut t = tt.lock().unwrap();
            t.dang_ban = false;
            match kq {
                Ok(s) => t.thong_bao = s,
                Err(e) => (t.thong_bao, t.loi) = (format!("Không làm được: {e}"), true),
            }
        });
    }

    fn doi_game(&mut self, g: &Game) {
        let lv = lv_cua(g);
        let clips: Vec<lt::Clip> = std::fs::read(lv.join("clips.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        self.gan = std::fs::read(lv.join("gan.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut t = self.tt.lock().unwrap();
        t.thong_bao = if clips.is_empty() { String::new() } else { format!("Đã nhớ {} file âm thanh từ lần trước.", clips.len()) };
        t.clips = clips;
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lồng tiếng");
        ui.label("Game có file thoại rời (wav / ogg / mp3 / flac / opus): quét > ghép chữ đã dịch > chọn giọng > lồng tiếng > cài (có sao lưu, gỡ được). Crimson Desert, Tomb Raider: dùng mục Làm tự động / Việt hóa game. Game đóng thoại trong gói riêng (Wwise, FSB, .bank...) cần công cụ riêng.");
        ui.add_space(6.0);
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
        let (ban, tb, loi, tien, so_clip) = {
            let t = self.tt.lock().unwrap();
            (t.dang_ban, t.thong_bao.clone(), t.loi, t.tien, t.clips.len())
        };
        if ban {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
        }
        let games = self.games.clone().unwrap_or_default();
        ui.horizontal_wrapped(|ui| {
            ui.label("Game:");
            let ten = self.chon.and_then(|i| games.get(i)).map(|g| g.ten.clone()).unwrap_or_else(|| "chọn game...".into());
            ui.add_enabled_ui(!ban, |ui| {
                egui::ComboBox::from_id_salt("lt_game").selected_text(ten).width(420.0).show_ui(ui, |ui| {
                    for (i, g) in games.iter().enumerate() {
                        ui.selectable_value(&mut self.chon, Some(i), &g.ten);
                    }
                });
            });
        });
        if self.chon != self.chon_cu {
            self.chon_cu = self.chon;
            if let Some(g) = self.chon.and_then(|i| games.get(i)) {
                self.doi_game(g);
            }
        }
        let Some(g) = self.chon.and_then(|i| games.get(i)).cloned() else { return };
        let lv = lv_cua(&g);

        the(ui, "1. QUÉT FILE ÂM THANH", |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(!ban, egui::Button::new("Quét file âm thanh")).clicked() {
                    let (dir, lv2) = (g.thu_muc.clone(), lv.clone());
                    self.nen("Đang quét file âm thanh", move |tt, dung| {
                        let t2 = tt.clone();
                        let clips = lt::quet(&dir, &move |s| t2.lock().unwrap().thong_bao = s.to_string(), dung);
                        if clips.is_empty() {
                            anyhow::bail!("không thấy file âm thanh rời nào (có thể game đóng thoại trong gói riêng)");
                        }
                        luu(&lv2, &clips);
                        let n = clips.len();
                        let gio: f64 = clips.iter().map(|c| c.giay).sum::<f64>() / 3600.0;
                        let ng: std::collections::HashSet<&str> = clips.iter().map(|c| c.nguoi.as_str()).collect();
                        let msg = format!("Có {n} file âm thanh ({gio:.1} giờ), {} nhóm người nói.", ng.len());
                        tt.lock().unwrap().clips = clips;
                        Ok(msg)
                    });
                }
                ui.label(format!("{so_clip} file đã quét"));
            });
        });

        the(ui, "2. GHÉP CHỮ ĐÃ DỊCH", |ui| {
            let co_ghep = self.tt.lock().unwrap().clips.iter().filter(|c| c.vi.is_some()).count();
            ui.label(format!("{co_ghep} file đã có câu tiếng Việt"));
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!ban && so_clip > 0, egui::Button::new("Ghép chữ + dịch câu còn thiếu (offline)")).clicked() {
                    let (dir, ten, lv2) = (g.thu_muc.clone(), g.ten.clone(), lv.clone());
                    self.nen("Đang ghép chữ", move |tt, dung| {
                        let mut v = ViecFile::moi(&dir, &ten);
                        v.viec.dung = dung.clone();
                        let d2 = dung.clone();
                        if v.kq_quet.is_none() {
                            let t2 = tt.clone();
                            v.quet(&move |s| t2.lock().unwrap().thong_bao = s.to_string(), &move || d2.load(Ordering::Relaxed));
                        }
                        v.doc_chon();
                        let mut clips = std::mem::take(&mut tt.lock().unwrap().clips);
                        let tim = |en: &str| v.viec.tim(en);
                        let mut n = lt::ghep_chu(&mut clips, &v.chu, &tim);
                        let thieu: Vec<String> = clips.iter().filter(|c| c.en.is_some() && c.vi.is_none()).filter_map(|c| c.en.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
                        if !thieu.is_empty() {
                            let bc = Bao(tt.clone());
                            let _ = v.viec.chay_offline(thieu, &bc);
                            let tim = |en: &str| v.viec.tim(en);
                            n = lt::ghep_chu(&mut clips, &v.chu, &tim);
                        }
                        let co = clips.iter().filter(|c| c.vi.is_some()).count();
                        luu(&lv2, &clips);
                        tt.lock().unwrap().clips = clips;
                        Ok(format!("Ghép được {n} file với câu chữ, {co} file đã có tiếng Việt. Thiếu thì xuất CSV, tự điền cột tieng_viet rồi nạp lại."))
                    });
                }
                if ui.add_enabled(!ban && so_clip > 0, egui::Button::new("Xuất bảng ghép (CSV)")).clicked() {
                    let p = duong_dan::desktop().join(format!("{}_long_tieng.csv", lt::thu_muc_lv(std::path::Path::new(""), &g.ten).file_name().unwrap().to_string_lossy()));
                    let r = lt::xuat_csv(&self.tt.lock().unwrap().clips, &p);
                    self.tt.lock().unwrap().thong_bao = match r {
                        Ok(n) => {
                            duong_dan::mo_explorer(&p);
                            format!("Đã xuất {n} dòng ra {}. Sửa cột nguoi / tieng_anh / tieng_viet rồi bấm Nạp.", p.display())
                        }
                        Err(e) => format!("Lỗi: {e}"),
                    };
                }
                if ui.add_enabled(!ban && so_clip > 0, egui::Button::new("Nạp bảng ghép (CSV)")).clicked() {
                    if let Some(p) = rfd::FileDialog::new().add_filter("CSV", &["csv"]).pick_file() {
                        let kq = lt::nap_csv(&mut self.tt.lock().unwrap().clips, &p);
                        let mut t = self.tt.lock().unwrap();
                        t.thong_bao = match kq {
                            Ok(n) => {
                                luu(&lv, &t.clips);
                                format!("Đã nạp {n} dòng.")
                            }
                            Err(e) => format!("Lỗi: {e}"),
                        };
                    }
                }
            });
        });

        let so = crate::them_giong::SO_LAN_LUU.load(Ordering::SeqCst);
        if so != self.kho_so {
            self.kho_so = so;
            self.kho = clone_giong::liet_ke();
        }
        the(ui, "3. GIỌNG TỪNG NGƯỜI", |ui| {
            self.them.nut(ui);
            let mut dem: HashMap<String, usize> = HashMap::new();
            for c in self.tt.lock().unwrap().clips.iter().filter(|c| c.vi.is_some()) {
                *dem.entry(c.nguoi.clone()).or_default() += 1;
            }
            let mut ds: Vec<(String, usize)> = dem.into_iter().collect();
            ds.sort_by_key(|(n, k)| (std::cmp::Reverse(*k), n.clone()));
            if ds.is_empty() {
                ui.label(RichText::new("Chưa có người nói nào có câu tiếng Việt.").weak());
            }
            let mut doi = false;
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for (ng, k) in ds.iter().take(40) {
                    ui.horizontal(|ui| {
                        ui.label(format!("{ng} ({k} câu)"));
                        let hien = self.gan.get(ng).cloned().unwrap_or_else(|| "nhái giọng gốc".into());
                        egui::ComboBox::from_id_salt(format!("lt_g_{ng}")).selected_text(hien).show_ui(ui, |ui| {
                            if ui.selectable_label(!self.gan.contains_key(ng), "nhái giọng gốc").clicked() {
                                self.gan.remove(ng);
                                doi = true;
                            }
                            for f in &self.kho {
                                if ui.selectable_label(self.gan.get(ng) == Some(f), f).clicked() {
                                    self.gan.insert(ng.clone(), f.clone());
                                    doi = true;
                                }
                            }
                        });
                    });
                }
            });
            if doi {
                let _ = std::fs::create_dir_all(&lv);
                let _ = std::fs::write(lv.join("gan.json"), serde_json::to_vec(&self.gan).unwrap_or_default());
            }
            ui.label(RichText::new("Kho giọng: tạo ở mục Clone giọng. Mặc định app nhái chính giọng gốc của người đó.").small().weak());
        });

        the(ui, "4. LỒNG TIẾNG VÀ CÀI", |ui| {
            let co_viet = self.tt.lock().unwrap().clips.iter().any(|c| c.vi.is_some());
            let da_lam = lv.join("LongTieng").is_dir();
            let da_cai = lv.join("sao_luu_goc").is_dir();
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!ban && co_viet, egui::Button::new(RichText::new("Lồng tiếng").strong())).clicked() {
                    let (dir, lv2, gan) = (g.thu_muc.clone(), lv.clone(), self.gan.clone());
                    let gan: HashMap<String, PathBuf> = gan.into_iter().map(|(k, v)| (k, clone_giong::kho_giong().join(v))).collect();
                    self.nen("Đang lồng tiếng", move |tt, dung| {
                        let clips = tt.lock().unwrap().clips.clone();
                        let (t1, t2) = (tt.clone(), tt.clone());
                        let kq = lt::long_tieng(
                            &dir,
                            &lv2,
                            &clips,
                            &gan,
                            dung,
                            &move |s| t1.lock().unwrap().thong_bao = s.to_string(),
                            &move |a, b, s| {
                                let mut t = t2.lock().unwrap();
                                t.tien = if b > 0 { a as f32 / b as f32 } else { 0.0 };
                                t.thong_bao = format!("Lồng tiếng{s} ({a}/{b})");
                            },
                        )?;
                        Ok(match kq {
                            None => "Đã dừng – bấm lại để làm tiếp.".to_string(),
                            Some(k) => format!("Xong {} file của {} người (thiếu giọng mẫu: {}). Bấm Cài vào game.", k.da_lam, k.nguoi, k.thieu_mau.len()),
                        })
                    });
                }
                if ui.add_enabled(!ban && da_lam, egui::Button::new("Cài vào game")).clicked() {
                    let (dir, lv2) = (g.thu_muc.clone(), lv.clone());
                    self.nen("Đang cài", move |_, _| Ok(format!("Đã chép {} file lồng tiếng vào game (bản gốc ở {}\\sao_luu_goc).", lt::cai(&dir, &lv2)?, lv2.display())));
                }
                if ui.add_enabled(!ban && da_cai, egui::Button::new("Gỡ – trả lại giọng gốc")).clicked() {
                    let (dir, lv2) = (g.thu_muc.clone(), lv.clone());
                    self.nen("Đang trả lại", move |_, _| Ok(format!("Đã trả lại {} file gốc.", lt::go(&dir, &lv2)?)));
                }
                if ui.add_enabled(ban, egui::Button::new("Dừng")).clicked() {
                    self.dung.store(true, Ordering::SeqCst);
                }
            });
            ui.label(RichText::new("Cần app VieNeu (GPU). Tắt game trước khi cài. Chạy lại = làm tiếp phần chưa xong. Nếu dịch còn thiếu, thêm bằng mục Làm tự động hoặc CSV.").small().weak());
        });

        if ban {
            ui.add(egui::ProgressBar::new(tien).show_percentage().animate(tien == 0.0));
        }
        if !tb.is_empty() {
            ui.label(RichText::new(tb).color(if loi { Color32::LIGHT_RED } else { Color32::YELLOW }));
        }
        let _ = dich::claude::co_khoa;
    }
}
