//! Trang Sửa bản dịch: xem / sửa từng câu trong bộ nhớ dịch của game hoặc phim, tìm & thay hàng loạt,
//! từ điển tên riêng + từ điển sửa bản dịch (áp dụng cho mọi lần dịch sau, cả offline lẫn Claude).

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText};
use serde_json::{Map, Value};

use crate::duong_dan;
use crate::viet_hoa::viec_dich::the_cua;

struct Muc {
    en: String,
    vi: String,
    goc: String,
}

#[derive(Default)]
struct TuDien {
    ten_rieng: String,
    sua: String,
    mo: bool,
}

pub struct TrangSuaDich {
    nguon: Vec<(String, PathBuf)>,
    chon: Option<usize>,
    muc: Vec<Muc>,
    tim: String,
    chi_sua: bool,
    chi_giong: bool,
    chi_the: bool,
    loc: Vec<usize>,
    khoa_loc: (String, bool, bool, bool, usize),
    doi: usize,
    thay_tim: String,
    thay_bang: String,
    ghi_td: bool,
    tb: String,
    td: TuDien,
}

fn tep_tu_dien() -> PathBuf {
    duong_dan::file_du_lieu("tu_dien.json")
}

/// Các file bộ nhớ dịch dạng {tiếng Anh: tiếng Việt} trên máy: game, phim.
fn tim_nguon() -> Vec<(String, PathBuf)> {
    let mut ra = Vec::new();
    let mut quet = |goc: PathBuf, nhan: &str| {
        for e in std::fs::read_dir(goc).into_iter().flatten().flatten() {
            let p = e.path().join("bo_nho_dich.json");
            if p.is_file() {
                let ten = e.file_name().to_string_lossy().into_owned();
                let kich = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) / 1024;
                if kich > 1 {
                    ra.push((format!("{nhan}: {ten} ({kich} KB)"), p));
                }
            }
        }
    };
    quet(duong_dan::viet_hoa_game(), "Game");
    quet(duong_dan::goc().join("DichGame_AI").join("phu_de_phim"), "Phim");
    ra
}

fn doc_muc(p: &Path) -> Result<Vec<Muc>, String> {
    let b = std::fs::read(p).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
    let o = v.as_object().ok_or("file không đúng dạng {tiếng Anh: tiếng Việt}")?;
    Ok(o.iter()
        .filter_map(|(k, v)| {
            Some(Muc { en: k.clone(), vi: v.as_str()?.to_string(), goc: v.as_str()?.to_string() })
        })
        .collect())
}

impl Default for TrangSuaDich {
    fn default() -> Self {
        Self {
            nguon: tim_nguon(),
            chon: None,
            muc: Vec::new(),
            tim: String::new(),
            chi_sua: false,
            chi_giong: false,
            chi_the: false,
            loc: Vec::new(),
            khoa_loc: Default::default(),
            doi: 0,
            thay_tim: String::new(),
            thay_bang: String::new(),
            ghi_td: true,
            tb: String::new(),
            td: TuDien::default(),
        }
    }
}

fn nap_tu_dien() -> (String, String) {
    let v: Value = std::fs::read(tep_tu_dien())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let ten = v
        .get("ten_rieng")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    let sua = v
        .get("sua_ban_dich")
        .and_then(Value::as_object)
        .map(|o| o.iter().filter_map(|(a, b)| Some(format!("{a} => {}", b.as_str()?))).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    (ten, sua)
}

fn luu_tu_dien(ten: &str, sua: &str) -> Result<(), String> {
    let mut v: Value = std::fs::read(tep_tu_dien())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let o = v.as_object_mut().ok_or("tu_dien.json hỏng")?;
    o.insert(
        "ten_rieng".into(),
        ten.lines().map(str::trim).filter(|l| !l.is_empty()).map(Value::from).collect::<Vec<_>>().into(),
    );
    let mut m = Map::new();
    for l in sua.lines() {
        if let Some((a, b)) = l.split_once("=>") {
            if !a.trim().is_empty() {
                m.insert(a.trim().into(), b.trim().into());
            }
        }
    }
    o.insert("sua_ban_dich".into(), Value::Object(m));
    duong_dan::ghi_file(&tep_tu_dien(), &serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    crate::dich::nap_lai_tu_dien();
    Ok(())
}

fn the(ui: &mut egui::Ui, tieu_de: &str, noi: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(tieu_de).color(Color32::from_rgb(64, 160, 255)).strong());
        noi(ui);
    });
    ui.add_space(6.0);
}

impl TrangSuaDich {
    fn mo(&mut self, i: usize) {
        self.chon = Some(i);
        self.tb = match doc_muc(&self.nguon[i].1) {
            Ok(m) => {
                let n = m.len();
                self.muc = m;
                format!("Đã mở {n} câu.")
            }
            Err(e) => {
                self.muc.clear();
                format!("Lỗi: {e}")
            }
        };
        self.doi += 1;
    }

    fn so_sua(&self) -> usize {
        self.muc.iter().filter(|m| m.vi != m.goc).count()
    }

    fn luu(&mut self) {
        let Some(i) = self.chon else { return };
        let p = self.nguon[i].1.clone();
        let sao = PathBuf::from(format!("{}.truoc_sua", p.display()));
        if !sao.exists() {
            let _ = std::fs::copy(&p, &sao);
        }
        let o: Map<String, Value> = self.muc.iter().map(|m| (m.en.clone(), Value::from(m.vi.clone()))).collect();
        self.tb = match serde_json::to_vec(&Value::Object(o))
            .map_err(|e| e.to_string())
            .and_then(|b| duong_dan::ghi_file(&p, &b).map_err(|e| e.to_string()))
        {
            Ok(()) => {
                let n = self.so_sua();
                for m in &mut self.muc {
                    m.goc = m.vi.clone();
                }
                format!("Đã lưu {n} câu đã sửa (bản cũ giữ ở .truoc_sua). Lần dịch / cài sau sẽ dùng bản này.")
            }
            Err(e) => format!("Lỗi lưu: {e}"),
        };
    }

    fn cap_nhat_loc(&mut self) {
        let k = (self.tim.clone(), self.chi_sua, self.chi_giong, self.chi_the, self.doi);
        if k == self.khoa_loc {
            return;
        }
        let q = self.tim.to_lowercase();
        self.loc = self
            .muc
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                (!self.chi_sua || m.vi != m.goc)
                    && (!self.chi_giong || m.vi == m.en)
                    && (!self.chi_the || the_cua(&m.en) != the_cua(&m.vi))
                    && (q.is_empty() || m.en.to_lowercase().contains(&q) || m.vi.to_lowercase().contains(&q))
            })
            .map(|(i, _)| i)
            .collect();
        self.khoa_loc = k;
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sửa bản dịch");
        ui.label("Chọn game / phim đã dịch > tìm câu > sửa trực tiếp > Lưu. Câu đã sửa được giữ nguyên ở các lần dịch sau; thuật ngữ sai lặp lại thì dùng Tìm & thay hoặc từ điển sửa bản dịch.");
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            ui.label("Nguồn:");
            let ten = self.chon.map(|i| self.nguon[i].0.clone()).unwrap_or_else(|| "chọn...".into());
            let mut moi = None;
            egui::ComboBox::from_id_salt("sd_nguon").selected_text(ten).width(360.0).show_ui(ui, |ui| {
                for (i, (t, _)) in self.nguon.iter().enumerate() {
                    if ui.selectable_label(self.chon == Some(i), t).clicked() {
                        moi = Some(i);
                    }
                }
            });
            if let Some(i) = moi {
                self.mo(i);
            }
            if ui.button("Mở file khác...").clicked() {
                if let Some(p) = rfd::FileDialog::new().add_filter("Bộ nhớ dịch", &["json"]).pick_file() {
                    self.nguon.push((format!("Tệp: {}", p.display()), p));
                    let i = self.nguon.len() - 1;
                    self.mo(i);
                }
            }
            if ui.button("Quét lại").clicked() {
                self.nguon = tim_nguon();
                self.chon = None;
                self.muc.clear();
                self.doi += 1;
            }
        });

        if !self.muc.is_empty() {
            let sua = self.so_sua();
            ui.horizontal_wrapped(|ui| {
                ui.label("Tìm:");
                ui.add(egui::TextEdit::singleline(&mut self.tim).desired_width(260.0).hint_text("chữ gốc hoặc bản dịch"));
                ui.checkbox(&mut self.chi_sua, "chỉ câu vừa sửa");
                ui.checkbox(&mut self.chi_giong, "chỉ câu giống bản gốc");
                ui.checkbox(&mut self.chi_the, "chỉ câu lệch thẻ ⚠");
                if ui.add_enabled(sua > 0, egui::Button::new(RichText::new(format!("Lưu ({sua} câu sửa)")).strong())).clicked() {
                    self.luu();
                }
            });
            self.cap_nhat_loc();
            ui.label(RichText::new(format!("{} / {} câu", self.loc.len(), self.muc.len())).small().weak());

            the(ui, "TÌM & THAY HÀNG LOẠT (trong bản dịch tiếng Việt)", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.thay_tim).desired_width(200.0).hint_text("chữ sai"));
                    ui.label("thành");
                    ui.add(egui::TextEdit::singleline(&mut self.thay_bang).desired_width(200.0).hint_text("chữ đúng"));
                    let n = if self.thay_tim.is_empty() { 0 } else { self.muc.iter().filter(|m| m.vi.contains(&self.thay_tim)).count() };
                    ui.label(format!("{n} câu có chữ này"));
                    ui.checkbox(&mut self.ghi_td, "nhớ cho các lần dịch sau");
                    if ui.add_enabled(n > 0, egui::Button::new("Thay tất cả")).clicked() {
                        for m in &mut self.muc {
                            if m.vi.contains(&self.thay_tim) {
                                m.vi = m.vi.replace(&self.thay_tim, &self.thay_bang);
                            }
                        }
                        self.doi += 1;
                        self.tb = format!("Đã thay trong {n} câu – nhớ bấm Lưu.");
                        if self.ghi_td {
                            let (ten, mut sua) = nap_tu_dien();
                            sua.push_str(&format!("\n{} => {}", self.thay_tim, self.thay_bang));
                            if let Err(e) = luu_tu_dien(&ten, &sua) {
                                self.tb = format!("Lỗi ghi từ điển: {e}");
                            }
                        }
                    }
                });
            });

            let dong = ui.text_style_height(&egui::TextStyle::Body);
            let cao = dong * 2.0 + 14.0;
            let n = self.loc.len();
            egui::ScrollArea::vertical().auto_shrink(false).max_height(ui.available_height() - 90.0).show_rows(ui, cao, n, |ui, r| {
                for k in r {
                    let i = self.loc[k];
                    let lech = the_cua(&self.muc[i].en) != the_cua(&self.muc[i].vi);
                    ui.allocate_ui(egui::vec2(ui.available_width(), cao), |ui| {
                        let m = &mut self.muc[i];
                        ui.add(egui::Label::new(RichText::new(&m.en).small().weak()).truncate());
                        ui.horizontal(|ui| {
                            let sua = m.vi != m.goc;
                            ui.add(egui::TextEdit::singleline(&mut m.vi).desired_width(ui.available_width() - 70.0));
                            if lech {
                                ui.label(RichText::new("⚠ thẻ").color(Color32::YELLOW)).on_hover_text("Thẻ {..} <..> [..] khác bản gốc – game có thể lỗi");
                            } else if sua {
                                ui.label(RichText::new("đã sửa").small().color(Color32::LIGHT_GREEN));
                            }
                        });
                    });
                }
            });
        }

        the(ui, "TỪ ĐIỂN (áp dụng cho mọi lần dịch sau)", |ui| {
            if ui.button(if self.td.mo { "Ẩn" } else { "Mở / sửa từ điển" }).clicked() {
                self.td.mo = !self.td.mo;
                if self.td.mo {
                    (self.td.ten_rieng, self.td.sua) = nap_tu_dien();
                }
            }
            if self.td.mo {
                ui.label("Tên riêng giữ nguyên tiếng Anh (mỗi dòng một tên):");
                ui.add(egui::TextEdit::multiline(&mut self.td.ten_rieng).desired_rows(4).desired_width(f32::INFINITY));
                ui.label("Sửa bản dịch (mỗi dòng: chữ sai => chữ đúng):");
                ui.add(egui::TextEdit::multiline(&mut self.td.sua).desired_rows(4).desired_width(f32::INFINITY));
                if ui.button("Lưu từ điển").clicked() {
                    self.tb = match luu_tu_dien(&self.td.ten_rieng, &self.td.sua) {
                        Ok(()) => "Đã lưu từ điển – dùng ngay cho các lần dịch sau (câu đã dịch rồi thì dùng Tìm & thay).".into(),
                        Err(e) => format!("Lỗi: {e}"),
                    };
                }
            }
        });
        if !self.tb.is_empty() {
            ui.label(if self.tb.starts_with("Lỗi") { RichText::new(&self.tb).color(Color32::LIGHT_RED) } else { RichText::new(&self.tb) });
        }
    }
}
