//! Trang Phụ đề phim: dán link / chọn file phim → AI nghe → dịch → .srt tiếng Việt → mở phim.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, RichText};

use crate::phu_de_phim::{self as pdp, ViecPhim};
use crate::viet_hoa::viec_dich::BaoCao;
use crate::{dich, duong_dan};

#[derive(Default)]
struct Tt {
    dang_ban: bool,
    thong_bao: String,
    loi: bool,
    xong: bool,
    /// tiến độ 0..1
    tien: f32,
    phim: Option<PathBuf>,
    srt: Option<PathBuf>,
    thu_muc: Option<PathBuf>,
    /// phim đã trộn giọng thuyết minh
    phim_tm: Option<PathBuf>,
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

pub struct TrangPhuDePhim {
    nguon: String,
    claude: bool,
    gioi_han: f64,
    tt: Arc<Mutex<Tt>>,
    dung: Arc<AtomicBool>,
    /// thuyết minh
    /// "Thiện Minh" (dựng sẵn) hoặc "kho:<tên file>" (giọng của bạn)
    giong: String,
    kho: Vec<String>,
    kho_so: usize,
    them: crate::them_giong::ThemGiong,
    theo_cam_xuc: bool,
    nhan_nha: bool,
    cuong_do: f64,
    chi_thu: bool,
    /// bảng cách đọc tên riêng (mỗi dòng: Tên => cách viết để đọc)
    ten_doc: String,
}

impl Default for TrangPhuDePhim {
    fn default() -> Self {
        Self {
            nguon: String::new(),
            claude: dich::claude::co_khoa(),
            gioi_han: 1.0,
            tt: Arc::new(Mutex::new(Tt::default())),
            dung: Arc::new(AtomicBool::new(false)),
            giong: "Thiện Minh".into(),
            kho: crate::viet_hoa::clone_giong::liet_ke(),
            kho_so: 0,
            them: Default::default(),
            theo_cam_xuc: true,
            nhan_nha: true,
            cuong_do: 0.7,
            chi_thu: false,
            ten_doc: crate::thuyet_minh::doc_ten_doc().iter().map(|(a, b)| format!("{a} => {b}")).collect::<Vec<_>>().join("\n"),
        }
    }
}

impl TrangPhuDePhim {
    pub fn dang_ky_tt(&self) {
        let t = self.tt.clone();
        crate::tien_trinh::dang_ky("phu_de_phim", crate::tien_trinh::co_dinh(self.dung.clone()), move || {
            let g = t.lock().unwrap();
            crate::tien_trinh::Anh {
                dang_ban: g.dang_ban,
                ten: "Phụ đề phim".into(),
                thong_bao: g.thong_bao.clone(),
                tien: (g.tien > 0.0).then_some(g.tien),
                loi: g.loi,
            }
        });
    }

    /// Thuyết minh bằng giọng đã chọn (dùng phụ đề đã tạo; chưa có thì báo tạo phụ đề trước).
    fn thuyet_minh(&mut self) {
        let nguon = self.nguon.trim().trim_matches('"').to_string();
        {
            let mut t = self.tt.lock().unwrap();
            if t.dang_ban {
                return;
            }
            if nguon.is_empty() {
                (t.thong_bao, t.loi) = ("Dán link phim hoặc chọn file phim trước.".into(), true);
                return;
            }
            *t = Tt { dang_ban: true, thong_bao: "Đang chuẩn bị thuyết minh...".into(), ..Default::default() };
        }
        let tuy = crate::thuyet_minh::Tuy {
            giong: self.giong.trim_start_matches("kho:").to_string(),
            theo_cam_xuc: self.theo_cam_xuc,
            nhan_nha: self.nhan_nha,
            cuong_do: self.cuong_do,
            thu_dau: self.chi_thu.then_some(10),
            tu_kho: self.giong.strip_prefix("kho:").map(String::from),
        };
        let (tt, dung) = (self.tt.clone(), self.dung.clone());
        dung.store(false, Ordering::SeqCst);
        std::thread::spawn(move || {
            let _thuc = duong_dan::GiuMayThuc::moi();
            let (a, b) = (tt.clone(), tt.clone());
            let kq = (|| -> anyhow::Result<Option<crate::thuyet_minh::KetQua>> {
                let v = ViecPhim::moi(&nguon)?;
                let phim = v.phim.as_ref().ok_or_else(|| anyhow::anyhow!("chưa có phim – bấm Tạo phụ đề tiếng Việt trước"))?.duong_dan.clone();
                if v.dong.is_empty() {
                    anyhow::bail!("chưa có phụ đề – bấm Tạo phụ đề tiếng Việt trước");
                }
                tt.lock().unwrap().phim = Some(phim.clone());
                let dong: Vec<(f64, f64, String)> = v.dong.iter().map(|(x, y, c)| (*x, *y, v.viec.tim(c).unwrap_or_default())).collect();
                crate::thuyet_minh::thuyet_minh(
                    &phim,
                    &dong,
                    &v.thu_muc_phim(),
                    &tuy,
                    &dung,
                    &move |s| a.lock().unwrap().thong_bao = s.to_string(),
                    &move |x, y| b.lock().unwrap().tien = if y > 0 { x as f32 / y as f32 } else { 0.0 },
                )
            })();
            let mut t = tt.lock().unwrap();
            t.dang_ban = false;
            match kq {
                Ok(Some(k)) => {
                    t.xong = true;
                    t.thong_bao = format!(
                        "Xong ✓ Đã thuyết minh {}/{} dòng{}{}. Phim: {}\nBáo cáo từng dòng (đọc lúc nào, dòng nào bị bỏ qua / cắt): {}",
                        k.so_dong,
                        k.tong_dong,
                        if k.bo_qua > 0 { format!(" • bỏ qua {} dòng chưa có bản dịch", k.bo_qua) } else { String::new() },
                        if k.so_cat > 0 { format!(" • {} dòng bị cắt cuối câu vì đọc quá dài", k.so_cat) } else { String::new() },
                        k.phim_moi.display(),
                        k.bao_cao.display()
                    );
                    t.phim_tm = Some(k.phim_moi);
                }
                Ok(None) => (t.thong_bao, t.loi) = ("Đã dừng – bấm lại để làm tiếp.".into(), true),
                Err(e) => (t.thong_bao, t.loi) = (format!("Không làm được: {e}"), true),
            }
            duong_dan::nhat_ky(&format!("Thuyết minh phim: {}", t.thong_bao));
        });
    }

    fn bat_dau(&mut self) {
        let nguon = self.nguon.trim().trim_matches('"').to_string();
        {
            let mut t = self.tt.lock().unwrap();
            if t.dang_ban {
                return;
            }
            if nguon.is_empty() {
                (t.thong_bao, t.loi) = ("Dán link phim hoặc chọn file phim trước.".into(), true);
                return;
            }
            if !pdp::la_link(&nguon) && !std::path::Path::new(&nguon).is_file() {
                (t.thong_bao, t.loi) = ("Không thấy file phim này.".into(), true);
                return;
            }
            *t = Tt { dang_ban: true, thong_bao: "Đang bắt đầu...".into(), ..Default::default() };
        }
        let (tt, dung, claude, gioi_han) = (self.tt.clone(), self.dung.clone(), self.claude, self.gioi_han);
        dung.store(false, Ordering::SeqCst);
        std::thread::spawn(move || {
            let _thuc = duong_dan::GiuMayThuc::moi();
            let kq = chay(&nguon, claude, gioi_han, &tt, &dung);
            let mut t = tt.lock().unwrap();
            t.dang_ban = false;
            match kq {
                Ok(()) => t.xong = true,
                Err(e) => (t.thong_bao, t.loi) = (format!("Không làm được: {e}"), true),
            }
            duong_dan::nhat_ky(&format!("Phụ đề phim: {}", t.thong_bao));
        });
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("Phụ đề phim");
        ui.label("• Dán link phim (YouTube và đa số trang video) hoặc chọn file phim trên máy.\n• App tải phim về, AI nghe lời thoại, dịch sang tiếng Việt và tạo phụ đề KHỚP TỪNG CÂU.\n• Phim 2 tiếng mất khoảng 5–15 phút. Xong app tự mở phim kèm phụ đề tiếng Việt.\n• Không dùng được cho Netflix / Disney+ (có khóa bản quyền).");
        ui.add_space(8.0);
        let (ban, tb, loi, xong, tien, phim, srt, thu_muc, phim_tm) = {
            let t = self.tt.lock().unwrap();
            (t.dang_ban, t.thong_bao.clone(), t.loi, t.xong, t.tien, t.phim.clone(), t.srt.clone(), t.thu_muc.clone(), t.phim_tm.clone())
        };
        if ban {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
        }
        ui.horizontal(|ui| {
            ui.add_enabled(
                !ban,
                egui::TextEdit::singleline(&mut self.nguon).desired_width(560.0).hint_text("link phim hoặc đường dẫn file..."),
            );
            if ui
                .add_enabled(!ban, egui::Button::new("Chọn file phim"))
                .clicked()
            {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Phim", &["mp4", "mkv", "avi", "mov", "webm", "m4v", "ts", "flv", "wmv", "mp3", "m4a", "wav"])
                    .pick_file()
                {
                    self.nguon = p.to_string_lossy().into_owned();
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Máy dịch:");
            ui.selectable_value(&mut self.claude, true, "Claude (hay, vài nghìn đồng/phim)");
            ui.selectable_value(&mut self.claude, false, "Offline (miễn phí)");
            if self.claude {
                ui.label("Giới hạn:");
                for g in [1.0, 2.0, 5.0] {
                    ui.selectable_value(&mut self.gioi_han, g, format!("{g:.0} USD"));
                }
            }
        });
        if self.claude && !dich::claude::co_khoa() {
            ui.label(RichText::new("Chưa có mã Claude – lưu mã trong Cài đặt, hoặc chọn Offline.").color(Color32::YELLOW));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!ban, egui::Button::new(RichText::new("Tạo phụ đề tiếng Việt").strong()))
                .clicked()
            {
                self.bat_dau();
            }
            if ui.add_enabled(ban, egui::Button::new("Dừng")).clicked() {
                self.dung.store(true, Ordering::SeqCst);
            }
            if ui
                .add_enabled(phim.is_some(), egui::Button::new("Mở phim"))
                .clicked()
            {
                if let Some(p) = &phim {
                    pdp::mo_phim(p, srt.as_deref());
                }
            }
            if ui
                .add_enabled(phim.is_some() || thu_muc.is_some(), egui::Button::new("Mở thư mục"))
                .clicked()
            {
                if let Some(p) = phim.as_ref().or(thu_muc.as_ref()) {
                    duong_dan::mo_explorer(p);
                }
            }
        });
        if ban {
            ui.add(egui::ProgressBar::new(tien).show_percentage().animate(tien == 0.0));
        }
        ui.add_space(8.0);
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("THUYẾT MINH TIẾNG VIỆT (giọng đọc có cảm xúc)").color(Color32::from_rgb(64, 160, 255)).strong());
            ui.label(RichText::new("Dùng phụ đề tiếng Việt đã tạo ở trên: đọc từng dòng bằng một giọng chuẩn, đặt đúng thời điểm, hạ tiếng gốc khi thuyết minh nói, xuất phim mới.").small().weak());
            ui.add_enabled_ui(!ban, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Giọng:");
                    let hien = match self.giong.strip_prefix("kho:") {
                        Some(f) => format!("{f} (giọng của bạn)"),
                        None => crate::thuyet_minh::GIONG
                            .iter()
                            .find(|g| g.0 == self.giong)
                            .map(|g| format!("{} ({})", g.0, g.1))
                            .unwrap_or_else(|| self.giong.clone()),
                    };
                    egui::ComboBox::from_id_salt("tm_giong").selected_text(hien).width(300.0).show_ui(ui, |ui| {
                        for (g, m) in crate::thuyet_minh::GIONG.iter() {
                            ui.selectable_value(&mut self.giong, g.to_string(), format!("{g} ({m})"));
                        }
                        if !self.kho.is_empty() {
                            ui.separator();
                            ui.label(RichText::new("Giọng của bạn (kho giọng)").small().weak());
                            for f in self.kho.clone() {
                                ui.selectable_value(&mut self.giong, format!("kho:{f}"), &f);
                            }
                        }
                    });
                    let so = crate::them_giong::SO_LAN_LUU.load(Ordering::SeqCst);
                    if so != self.kho_so {
                        self.kho_so = so;
                        self.kho = crate::viet_hoa::clone_giong::liet_ke();
                    }
                    self.them.nut(ui);
                });
                ui.checkbox(&mut self.theo_cam_xuc, "Giọng theo cảm xúc: dùng mẫu bình thường / gấp rút / buồn của giọng này theo từng câu");
                ui.checkbox(&mut self.nhan_nha, "Nhấn nhá theo câu: chỉnh to nhỏ, nhanh chậm, cao thấp theo dấu câu và từ khóa");
                if self.nhan_nha {
                    ui.horizontal(|ui| {
                        ui.label("Độ mạnh nhấn nhá:");
                        ui.add(egui::Slider::new(&mut self.cuong_do, 0.3..=1.0).fixed_decimals(1));
                    });
                }
                ui.checkbox(&mut self.chi_thu, "Chỉ làm thử 10 dòng đầu (phần còn lại giữ nguyên tiếng gốc – bỏ chọn để làm cả phim)");
                ui.label(RichText::new("Cách đọc tên riêng: tên không ghi ở đây được đọc nguyên văn tiếng Anh. Muốn đọc khác thì thêm dòng `Tên => cách viết`, vd `Jess => Giét`.").small().weak());
                if ui.add(egui::TextEdit::multiline(&mut self.ten_doc).desired_rows(3).desired_width(f32::INFINITY).hint_text("Jess => Giét")).changed() {
                    let _ = std::fs::write(crate::thuyet_minh::tep_ten_doc(), &self.ten_doc);
                }
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(!ban, egui::Button::new(RichText::new("Tạo thuyết minh").strong())).clicked() {
                    self.thuyet_minh();
                }
                if ui.add_enabled(phim_tm.is_some(), egui::Button::new("Mở phim thuyết minh")).clicked() {
                    if let Some(p) = &phim_tm {
                        pdp::mo_phim(p, None);
                    }
                }
            });
        });
        if !tb.is_empty() {
            ui.add_space(6.0);
            let m = if loi {
                Color32::LIGHT_RED
            } else if xong {
                Color32::LIGHT_GREEN
            } else {
                Color32::YELLOW
            };
            ui.label(RichText::new(tb).color(m));
        }
    }
}

fn chay(
    nguon: &str,
    claude: bool,
    gioi_han: f64,
    tt: &Arc<Mutex<Tt>>,
    dung: &Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let mut v = ViecPhim::moi(nguon)?;
    v.viec.dung = dung.clone();
    tt.lock().unwrap().thu_muc = Some(v.thu_muc_phim());
    let bat_dau = std::time::Instant::now();
    let dat = |s: String| tt.lock().unwrap().thong_bao = s;
    let tien_do = |buoc: &str, a: f64, b: f64| {
        let mut t = tt.lock().unwrap();
        t.tien = if b > 0.0 { (a / b) as f32 } else { 0.0 };
        match buoc {
            "tai" => t.thong_bao = format!("Đang tải phim: {a:.0}%"),
            _ => {
                let con = if a > 30.0 { bat_dau.elapsed().as_secs_f64() / a * (b - a) } else { 0.0 };
                t.thong_bao = format!("AI đang nghe lời thoại: {:.0}/{:.0} phút phim{}", a / 60.0, b / 60.0,
                    if con > 0.0 { format!(" • còn khoảng {:.0} phút", con / 60.0) } else { String::new() });
            }
        }
    };
    v.tai_va_nghe(None, dung, &|s| dat(s.to_string()), &tien_do)?;
    tt.lock().unwrap().phim = v.phim.as_ref().map(|p| p.duong_dan.clone());
    dat(format!("Đã nghe xong {} dòng – đang dịch...", v.dong.len()));
    let ds = v.cau_can_dich();
    let bc = Bao(tt.clone());
    if claude && dich::claude::co_khoa() {
        v.viec.chay_claude(ds, gioi_han, &bc)?;
    } else {
        v.viec.chay_offline(ds, &bc)?;
    }
    let _ = v.viec.luu();
    if dung.load(Ordering::Relaxed) {
        anyhow::bail!("đã dừng – bấm lại để làm tiếp");
    }
    let srt = v.xuat_srt()?;
    let con = v.cau_can_dich().len();
    {
        let mut t = tt.lock().unwrap();
        t.srt = Some(srt.clone());
        t.thong_bao = format!(
            "Xong ✓ Phụ đề tiếng Việt: {}{}\nĐang mở phim… Nếu không thấy phụ đề: mở bằng VLC / PotPlayer, hoặc kéo file .srt vào cửa sổ phim.",
            srt.display(),
            if con > 0 { format!("\n⚠ {con} dòng chưa dịch được (giữ nguyên) – bấm Tạo phụ đề lần nữa để dịch tiếp.") } else { String::new() }
        );
    }
    if std::env::var_os("VIET_HOA_KHONG_MO").is_none() {
        if let Some(p) = &v.phim {
            pdp::mo_phim(&p.duong_dan, Some(&srt));
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    /// Cả chuỗi (nghe + dịch offline + .srt): FILE=<audio/video> VIET_HOA_KHONG_MO=1 cargo test --release -- --ignored chuoi_that --nocapture
    #[test]
    #[ignore]
    fn chuoi_that() {
        let f = std::env::var("FILE").unwrap();
        let tt = Arc::new(Mutex::new(Tt::default()));
        chay(&f, false, 1.0, &tt, &Arc::new(AtomicBool::new(false))).unwrap();
        let t = tt.lock().unwrap();
        println!("{}", t.thong_bao);
        let srt = std::fs::read_to_string(t.srt.as_ref().unwrap()).unwrap();
        println!("{}", &srt[..srt.len().min(700)]);
        assert!(srt.contains("-->"));
    }
}
