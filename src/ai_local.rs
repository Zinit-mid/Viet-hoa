//! AI chạy trên máy (Ollama, mặc định qwen3:14b). Dùng cho: dịch câu còn thiếu, đoán giới tính nhân vật,
//! giải thích lỗi. RTX 3060 chỉ có 12 GB nên AI và lồng tiếng (VieNeu) KHÔNG được chạy cùng lúc:
//! - `KhoaLongTieng::vao()` (gọi đầu mọi việc lồng tiếng) tắt AI khỏi VRAM và chặn AI cho tới khi lồng xong;
//! - mọi lời gọi AI bị từ chối khi đang lồng tiếng; lồng tiếng bị từ chối khi AI đang làm việc.

use anyhow::{anyhow, bail, Result};
use eframe::egui::{self, Color32, RichText};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::cau_hinh;

const MAC_DINH: &str = "qwen3:14b";
const URL: &str = "http://127.0.0.1:11434";

static DANG_LONG_TIENG: AtomicBool = AtomicBool::new(false);
static DANG_DUNG: AtomicUsize = AtomicUsize::new(0);

pub fn mo_hinh() -> String {
    cau_hinh::doc_khoa("ai_local_mo_hinh.txt").unwrap_or_else(|| MAC_DINH.to_string())
}

/// Ollama đang chạy và trả lời.
pub fn san_sang() -> bool {
    ureq::get(&format!("{URL}/api/tags")).timeout(Duration::from_secs(2)).call().is_ok()
}

/// Giữ suốt thời gian lồng tiếng: AI bị tắt và chặn.
pub struct KhoaLongTieng;

impl KhoaLongTieng {
    pub fn vao() -> Result<Self> {
        DANG_LONG_TIENG.store(true, Ordering::SeqCst);
        if DANG_DUNG.load(Ordering::SeqCst) > 0 {
            DANG_LONG_TIENG.store(false, Ordering::SeqCst);
            bail!("AI local đang làm việc – đợi nó xong (hoặc đóng cửa sổ AI) rồi hãy lồng tiếng, kẻo hai bên tranh bộ nhớ card đồ họa");
        }
        let _ = xa_vram();
        Ok(Self)
    }
}

impl Drop for KhoaLongTieng {
    fn drop(&mut self) {
        DANG_LONG_TIENG.store(false, Ordering::SeqCst);
    }
}

struct DangDung;

impl DangDung {
    fn vao() -> Result<Self> {
        DANG_DUNG.fetch_add(1, Ordering::SeqCst);
        if DANG_LONG_TIENG.load(Ordering::SeqCst) {
            DANG_DUNG.fetch_sub(1, Ordering::SeqCst);
            bail!("đang lồng tiếng – AI local tạm nghỉ để nhường card đồ họa");
        }
        Ok(Self)
    }
}

impl Drop for DangDung {
    fn drop(&mut self) {
        DANG_DUNG.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Trả hết VRAM mà mô hình đang giữ.
pub fn xa_vram() -> Result<()> {
    ureq::post(&format!("{URL}/api/generate"))
        .timeout(Duration::from_secs(10))
        .send_json(serde_json::json!({"model": mo_hinh(), "keep_alive": 0}))?;
    Ok(())
}

fn hoi(he_thong: &str, noi_dung: &str) -> Result<String> {
    let _g = DangDung::vao()?;
    let than = serde_json::json!({
        "model": mo_hinh(),
        "stream": false,
        "think": false,
        "keep_alive": "2m",
        "options": {"temperature": 0.2, "num_ctx": 4096},
        "messages": [
            {"role": "system", "content": he_thong},
            {"role": "user", "content": noi_dung},
        ],
    });
    let tra: serde_json::Value = ureq::post(&format!("{URL}/api/chat"))
        .timeout(Duration::from_secs(300))
        .send_json(than)
        .map_err(|e| anyhow!("không gọi được Ollama ({e}) – mở app Ollama / AI Local Studio trước"))?
        .into_json()?;
    let mut s = tra["message"]["content"].as_str().unwrap_or("").to_string();
    if let (Some(a), Some(b)) = (s.find("<think>"), s.find("</think>")) {
        if b > a {
            s.replace_range(a..b + 8, "");
        }
    }
    Ok(s.trim().to_string())
}

/// Dịch một lô câu Anh → Việt (giữ thẻ do `offline_lo` lo).
pub fn dich_lo(cau: &[String]) -> Result<Vec<String>> {
    let vao = serde_json::to_string(cau)?;
    let r = hoi(
        "Bạn là người dịch game từ tiếng Anh sang tiếng Việt, giọng tự nhiên, ngắn gọn. Giữ nguyên tên riêng. \
         Nhận một mảng JSON các câu; trả về DUY NHẤT một mảng JSON cùng số phần tử, cùng thứ tự, mỗi phần tử là bản dịch.",
        &vao,
    )?;
    let (a, b) = (r.find('['), r.rfind(']'));
    let (Some(a), Some(b)) = (a, b) else { bail!("AI trả lời không đúng dạng") };
    let v: Vec<String> = serde_json::from_str(&r[a..=b])?;
    if v.len() != cau.len() {
        bail!("AI trả {} câu, cần {}", v.len(), cau.len());
    }
    Ok(v)
}

/// Đoán giới tính nhân vật từ tên + vài câu thoại. Some(true) = nữ, Some(false) = nam, None = không rõ.
pub fn gioi_tinh(ten: &str, cau_mau: &[String]) -> Result<Option<bool>> {
    let mut nd = format!("Nhân vật trong game Crimson Desert, mã tên: {ten}.\nMột vài câu thoại:\n");
    for c in cau_mau {
        nd.push_str(&format!("- {c}\n"));
    }
    let r = hoi(
        "Đoán giới tính nhân vật game. Chỉ trả lời đúng MỘT từ: nam, nu, hoac khong_ro. Không giải thích.",
        &nd,
    )?
    .to_lowercase();
    Ok(if r.contains("khong") {
        None
    } else if r.contains("nu") || r.contains("nữ") {
        Some(true)
    } else if r.contains("nam") {
        Some(false)
    } else {
        None
    })
}

fn mang_json(r: &str, n: usize) -> Result<Vec<String>> {
    let (a, b) = (r.find('['), r.rfind(']'));
    let (Some(a), Some(b)) = (a, b) else { bail!("AI trả lời không đúng dạng") };
    let v: Vec<String> = serde_json::from_str(&r[a..=b])?;
    if v.len() != n {
        bail!("AI trả {} câu, cần {n}", v.len());
    }
    Ok(v)
}

/// Soát bản dịch: cặp (tiếng Anh, bản dịch hiện có) → bản dịch đã sửa (tốt thì giữ nguyên).
pub fn sua_dich(cap: &[(String, String)]) -> Result<Vec<String>> {
    let vao = serde_json::to_string(&cap.iter().map(|(en, vi)| serde_json::json!({"en": en, "vi": vi})).collect::<Vec<_>>())?;
    let r = hoi(
        "Bạn là biên tập viên dịch game / phụ đề phim Anh-Việt. Nhận mảng JSON các đối tượng {en, vi}. Với mỗi cặp: nếu bản dịch vi đã tốt thì giữ NGUYÊN; \
         nếu sai nghĩa, cứng, còn sót tiếng Anh hoặc chưa dịch thì sửa lại tự nhiên, ngắn gọn. Giữ nguyên tên riêng và mọi thẻ như {..}, <..>, [..], %s. \
         Trả về DUY NHẤT một mảng JSON các chuỗi tiếng Việt, cùng số phần tử, cùng thứ tự.",
        &vao,
    )?;
    mang_json(&r, cap.len())
}

/// Rút gọn các câu thuyết minh quá dài: (câu Việt, số giây được đọc) → câu ngắn hơn, giữ ý chính.
pub fn rut_gon(cau: &[(String, f64)]) -> Result<Vec<String>> {
    let vao = serde_json::to_string(&cau.iter().map(|(c, g)| serde_json::json!({"vi": c, "giay": (g * 10.0).round() / 10.0})).collect::<Vec<_>>())?;
    let r = hoi(
        "Bạn viết lời thuyết minh phim tiếng Việt. Nhận mảng JSON {vi, giay}. Mỗi câu phải đọc xong trong số giây cho phép (người đọc khoảng 13 chữ cái mỗi giây). \
         Hãy rút gọn câu vi cho vừa, giữ ý chính và giọng văn, giữ tên riêng. Trả về DUY NHẤT một mảng JSON các chuỗi, cùng số phần tử, cùng thứ tự.",
        &vao,
    )?;
    mang_json(&r, cau.len())
}

pub fn giai_thich_loi(loi: &str) -> Result<String> {
    hoi(
        "Bạn giúp người dùng không rành kỹ thuật của app Việt hóa game. Đọc thông báo lỗi và trả lời bằng tiếng Việt, \
         ngắn gọn: (1) lỗi nghĩa là gì, (2) 2-3 bước cụ thể nên làm. Không bịa tên nút không có.",
        loi,
    )
}

// ---------------- trang giao diện ----------------

#[derive(Default)]
struct Tt {
    dang_ban: bool,
    thong_bao: String,
    loi: bool,
    ket_qua: String,
}

pub struct TrangAi {
    games: Vec<crate::viet_hoa::tim_game::Game>,
    da_tim: bool,
    dang_tim: Arc<Mutex<Option<Vec<crate::viet_hoa::tim_game::Game>>>>,
    chon: usize,
    tt: Arc<Mutex<Tt>>,
    mo_hinh: String,
    loi_dan: String,
}

impl Default for TrangAi {
    fn default() -> Self {
        Self { games: Vec::new(), da_tim: false, dang_tim: Arc::new(Mutex::new(None)), chon: 0, tt: Arc::new(Mutex::new(Tt::default())), mo_hinh: mo_hinh(), loi_dan: String::new() }
    }
}

fn csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\"").replace(['\r', '\n'], " "))
}

fn gioi_tinh_giong(ten_giong: &str) -> Option<bool> {
    let l = ten_giong.to_lowercase();
    let t: Vec<&str> = l.split(['_', '.', '-', ' ']).collect();
    if t.iter().any(|x| *x == "nu" || *x == "gai") {
        Some(true)
    } else if t.iter().any(|x| *x == "nam" || *x == "trai") {
        Some(false)
    } else {
        None
    }
}

type Hang = (String, Vec<String>, String, Option<f64>, bool);

/// Gom (người nói, câu mẫu, giọng đang gán, cao độ gốc, khóa tay) của một game.
fn gom_nhan_vat(g: &crate::viet_hoa::tim_game::Game, tt: &Arc<Mutex<Tt>>) -> Result<Vec<Hang>> {
    use crate::viet_hoa::crimson_desert::CrimsonDesert;
    let lv = crate::viet_hoa::tomb_raider::thu_muc_lam_viec(&g.thu_muc);
    let mau_cau = |ds: Vec<String>| -> Vec<String> {
        ds.into_iter().filter(|c| c.chars().count() > 12).take(6).map(|c| c.chars().take(160).collect()).collect()
    };
    let mut ra: Vec<Hang> = Vec::new();
    if g.ten.to_lowercase().contains("crimson") {
        tt.lock().unwrap_or_else(|e| e.into_inner()).thong_bao = "Đang đọc danh sách nhân vật...".into();
        let cd = CrimsonDesert::nap(&g.thu_muc)?;
        let k = cd.ke_hoach_long_tieng('A', &|_| {})?;
        let pb: serde_json::Value = std::fs::read(lv.join("giong_rieng").join("phan_bo.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let tay = pb.get("_tay").map(|v| v.to_string()).unwrap_or_default();
        let hz: std::collections::BTreeMap<String, Option<f64>> = std::fs::read(lv.join("A").join("_cao_do_goc.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        for (ten, ds) in &k {
            let gan = pb.get("A").and_then(|a| a.get(ten)).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let mau = mau_cau(ds.iter().filter_map(|c| c.en.clone()).collect());
            ra.push((ten.clone(), mau, gan, hz.get(ten).copied().flatten(), tay.contains(&format!("\"{ten}\""))));
        }
    } else {
        // game khác: dùng danh sách file âm thanh đã quét ở mục Lồng tiếng game
        let d = crate::viet_hoa::long_tieng_chung::thu_muc_lv(&lv, &g.ten);
        let clips: Vec<crate::viet_hoa::long_tieng_chung::Clip> = std::fs::read(d.join("clips.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if clips.is_empty() {
            bail!("game này chưa quét file âm thanh – vào Lồng tiếng > Lồng tiếng game, chọn game rồi bấm Quét file âm thanh, sau đó quay lại đây");
        }
        let gan: std::collections::HashMap<String, String> =
            std::fs::read(d.join("gan.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let mut theo: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for c in &clips {
            if let Some(en) = &c.en {
                theo.entry(c.nguoi.clone()).or_default().push(en.clone());
            }
        }
        for (ten, ds) in theo {
            let v = gan.get(&ten).cloned().unwrap_or_default();
            ra.push((ten, mau_cau(ds), v, None, false));
        }
    }
    Ok(ra)
}

/// Báo cáo: AI nghĩ nhân vật nam / nữ, so với giọng đang gán. Ghi CSV ra Desktop (không tự đổi gì). Dùng cho mọi game.
fn kiem_tra_gioi_tinh(tt: &Arc<Mutex<Tt>>, g: &crate::viet_hoa::tim_game::Game) -> Result<String> {
    let bao = |s: String| tt.lock().unwrap_or_else(|e| e.into_inner()).thong_bao = s;
    let hang = gom_nhan_vat(g, tt)?;
    let tong = hang.len();
    if tong == 0 {
        bail!("không thấy nhân vật nào có câu thoại tiếng Anh");
    }
    let mut dong = vec!["nguoi_noi,ai_doan,giong_dang_gan,gioi_tinh_giong,cao_do_goc_hz,khoa_tay,nghi_sai,cau_mau".to_string()];
    let mut nghi = 0usize;
    for (i, (ten, mau, gan, hz, khoa)) in hang.iter().enumerate() {
        if DANG_LONG_TIENG.load(Ordering::SeqCst) {
            bail!("đang lồng tiếng – dừng kiểm tra AI");
        }
        bao(format!("AI đang xem nhân vật {}/{tong}: {ten}", i + 1));
        let ai = if mau.is_empty() { None } else { gioi_tinh(ten, mau)? };
        let gt_giong = gioi_tinh_giong(gan);
        let sai = matches!((ai, gt_giong), (Some(a), Some(b)) if a != b);
        if sai {
            nghi += 1;
        }
        let txt = |o: Option<bool>| match o {
            Some(true) => "nu",
            Some(false) => "nam",
            None => "?",
        };
        dong.push(format!(
            "{},{},{},{},{},{},{},{}",
            csv(ten),
            txt(ai),
            csv(gan),
            txt(gt_giong),
            hz.map(|x| format!("{x:.0}")).unwrap_or_default(),
            if *khoa { "co" } else { "" },
            if sai { "NGHI SAI" } else { "" },
            csv(&mau.first().cloned().unwrap_or_default())
        ));
    }
    let p = crate::duong_dan::desktop().join("AI_kiem_tra_gioi_tinh.csv");
    let mut b = vec![0xEF, 0xBB, 0xBF];
    b.extend(dong.join("\r\n").as_bytes());
    crate::duong_dan::ghi_file(&p, &b)?;
    crate::duong_dan::mo_explorer(&p);
    Ok(format!("Xong {tong} nhân vật của {}, {nghi} nghi sai giọng. Báo cáo: Desktop\\AI_kiem_tra_gioi_tinh.csv (cột nghi_sai). App không tự đổi gì – bạn xem rồi gán lại ở mục Giọng mẫu.", g.ten))
}

impl TrangAi {
    pub fn dang_ky_tt(&self) {
        let t = self.tt.clone();
        crate::tien_trinh::dang_ky("ai_local", None, move || {
            let g = t.lock().unwrap_or_else(|e| e.into_inner());
            crate::tien_trinh::Anh { dang_ban: g.dang_ban, ten: "AI local".into(), thong_bao: g.thong_bao.clone(), tien: None, loi: g.loi }
        });
    }

    fn nen(&self, ten: &str, f: impl FnOnce(&Arc<Mutex<Tt>>) -> Result<String> + Send + 'static) {
        let tt = self.tt.clone();
        {
            let mut g = tt.lock().unwrap_or_else(|e| e.into_inner());
            if g.dang_ban {
                return;
            }
            g.dang_ban = true;
            g.loi = false;
            g.thong_bao = ten.to_string();
        }
        std::thread::spawn(move || {
            let kq = f(&tt);
            let mut g = tt.lock().unwrap_or_else(|e| e.into_inner());
            g.dang_ban = false;
            match kq {
                Ok(s) => {
                    g.thong_bao = s.clone();
                    g.ket_qua = s;
                }
                Err(e) => {
                    g.thong_bao = format!("Lỗi: {e}");
                    g.loi = true;
                }
            }
        });
    }

    pub fn ve(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI local");
        ui.label("AI chạy ngay trên máy (Ollama). Giúp dịch câu còn thiếu, đoán giới tính nhân vật, giải thích lỗi. Khi lồng tiếng, app tự tắt AI để nhường card đồ họa.");
        ui.add_space(8.0);
        let (ban, tb, loi, kq) = {
            let g = self.tt.lock().unwrap_or_else(|e| e.into_inner());
            (g.dang_ban, g.thong_bao.clone(), g.loi, g.ket_qua.clone())
        };
        if ban {
            ui.ctx().request_repaint_after(Duration::from_millis(400));
        }
        let long_tieng = DANG_LONG_TIENG.load(Ordering::SeqCst);
        crate::kieu::khoi(ui, "KẾT NỐI", |ui| {
            if long_tieng {
                ui.label(RichText::new("Đang lồng tiếng – AI đã bị tắt và tạm khóa.").color(Color32::YELLOW));
            }
            ui.horizontal_wrapped(|ui| {
                ui.label("Mô hình:");
                ui.add(egui::TextEdit::singleline(&mut self.mo_hinh).desired_width(200.0));
                if ui.button("Lưu").clicked() && !self.mo_hinh.trim().is_empty() {
                    let _ = cau_hinh::luu_khoa("ai_local_mo_hinh.txt", &self.mo_hinh);
                }
                if ui.add_enabled(!ban, egui::Button::new("Kiểm tra kết nối")).clicked() {
                    self.nen("Đang kiểm tra...", |_| {
                        if san_sang() {
                            Ok(format!("Ollama đang chạy. Mô hình: {}", mo_hinh()))
                        } else {
                            Err(anyhow!("chưa thấy Ollama – mở app Ollama hoặc AI Local Studio rồi thử lại"))
                        }
                    });
                }
                if ui.add_enabled(!ban, egui::Button::new("Xả sạch VRAM")).clicked() {
                    self.nen("Đang xả VRAM...", |_| {
                        xa_vram()?;
                        Ok("Đã trả lại bộ nhớ card đồ họa.".into())
                    });
                }
            });
        });
        if !self.da_tim {
            self.da_tim = true;
            let o = self.dang_tim.clone();
            std::thread::spawn(move || *o.lock().unwrap_or_else(|e| e.into_inner()) = Some(crate::viet_hoa::tim_game::tim_tat_ca()));
        }
        if let Some(g) = self.dang_tim.lock().unwrap_or_else(|e| e.into_inner()).take() {
            self.games = g;
        }
        crate::kieu::khoi(ui, "KIỂM TRA GIỚI TÍNH NHÂN VẬT (mọi game)", |ui| {
            ui.label("Chọn game. AI đọc tên + vài câu thoại từng nhân vật rồi so với giọng đang gán, xuất danh sách nghi sai ra Desktop. Crimson Desert đọc thẳng từ game; game khác cần đã quét file âm thanh ở Lồng tiếng game.");
            if self.games.is_empty() {
                ui.label("Đang tìm game trên máy...");
                ui.ctx().request_repaint_after(Duration::from_millis(300));
            } else {
                if self.chon >= self.games.len() {
                    self.chon = 0;
                }
                let ten = self.games[self.chon].ten.clone();
                egui::ComboBox::from_id_salt("ai_game").selected_text(ten).width(360.0).show_ui(ui, |ui| {
                    for (i, g) in self.games.iter().enumerate() {
                        ui.selectable_value(&mut self.chon, i, &g.ten);
                    }
                });
                if ui.add_enabled(!ban && !long_tieng, egui::Button::new("Bắt đầu kiểm tra")).clicked() {
                    let g = self.games[self.chon].clone();
                    self.nen("Đang kiểm tra giới tính...", move |tt| kiem_tra_gioi_tinh(tt, &g));
                }
            }
        });
        crate::kieu::khoi(ui, "GIẢI THÍCH LỖI", |ui| {
            ui.label("Dán thông báo lỗi vào đây, AI giải thích bằng tiếng Việt.");
            ui.add(egui::TextEdit::multiline(&mut self.loi_dan).desired_rows(4).desired_width(f32::INFINITY));
            if ui.add_enabled(!ban && !long_tieng && !self.loi_dan.trim().is_empty(), egui::Button::new("Giải thích")).clicked() {
                let l = self.loi_dan.clone();
                self.nen("AI đang đọc lỗi...", move |_| giai_thich_loi(&l));
            }
        });
        if !tb.is_empty() {
            ui.label(if loi { RichText::new(&tb).color(Color32::from_rgb(255, 131, 137)) } else { RichText::new(&tb) });
        }
        let _ = kq;
    }
}
