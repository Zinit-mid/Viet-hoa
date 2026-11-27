//! Lõi Việt hóa (giống ViecDich trong viet_hoa_cd.py của app cũ), dùng chung cho mọi game
//! và phụ đề phim:
//!   danh sách câu → bỏ trùng / bỏ câu đã dịch → Claude theo lô JSON (4 luồng, giữ tên riêng,
//!   kiểm thẻ, lô hỏng thì chia đôi) | Offline (giữ thẻ <..> {..} [EMPTY], xuống dòng)
//!   → bộ nhớ {tiếng Anh: tiếng Việt} (ghi .tam rồi đổi tên).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{anyhow, Result};
use regex::Regex;
use std::sync::OnceLock;

use crate::dich::{claude, offline};
use crate::duong_dan;

pub const KY_TU_MOI_LO: usize = 6000;
pub const CAU_MOI_LO: usize = 40;
pub const SO_LUONG: usize = 4;
/// Giá mặc định USD / 1 triệu token (vào, ra)
pub const GIA_MAC_DINH: [f64; 2] = [0.10, 0.50];

fn re_the() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"<[^<>]*>|\{[^{}]*\}|\[EMPTY\]").unwrap())
}

/// Chuỗi không có chữ để dịch (chỉ thẻ, số, ký hiệu).
pub fn khong_can_dich(s: &str) -> bool {
    let t = re_the().replace_all(s, "");
    let t = t.trim();
    t.is_empty() || !Regex::new(r"[A-Za-z]{2,}").unwrap().is_match(t)
}

/// Chuỗi là câu / đoạn văn (mô tả) chứ không phải tên hay nhãn ngắn.
pub fn la_cau(s: &str) -> bool {
    let t = re_the().replace_all(s, " ");
    let t = t.trim();
    if s.contains("<br/>") || t.chars().count() >= 70 {
        return true;
    }
    t.split_whitespace().count() >= 4
        && (t.ends_with(['.', '!', '?', '…', '"']) || t.contains(". "))
}

/// Các thẻ trong chuỗi (đã xếp) – bản dịch phải có đúng các thẻ này.
pub fn the_cua(s: &str) -> Vec<String> {
    let mut v: Vec<String> = re_the()
        .find_iter(s)
        .map(|m| m.as_str().to_string())
        .collect();
    v.sort();
    v
}

// ---------------- bảng tên riêng (giữ nguyên khi dịch) ----------------

#[derive(Default, Clone)]
pub struct BangTen {
    ten: HashSet<String>,
    dai: usize,
}

const TU_THUONG: &[&str] = &[
    "Talk",
    "Map",
    "Quest",
    "Quests",
    "Inventory",
    "Continue",
    "Settings",
    "Options",
    "Yes",
    "No",
    "Gold",
    "Silver",
    "Food",
    "Water",
    "Wood",
    "Stone",
    "Iron",
    "Horse",
    "Sword",
    "Shield",
    "Bow",
    "Help",
    "Back",
    "Close",
    "Cancel",
    "Confirm",
    "Accept",
    "Reward",
    "Rewards",
    "Level",
    "None",
];

impl BangTen {
    /// Tạo từ các chuỗi trong file tên (NPC, địa danh, vật phẩm...): giữ chuỗi ngắn viết hoa đầu.
    pub fn tao<'a>(cac_chuoi: impl IntoIterator<Item = &'a str>) -> Self {
        let mut ten = HashSet::new();
        for v in cac_chuoi {
            let v = re_the().replace_all(v, "");
            let v = v.trim();
            let n = v.chars().count();
            if (2..=40).contains(&n)
                && !v.contains('\n')
                && !v.ends_with(['.', '!', '?', ':'])
                && v.chars().next().is_some_and(char::is_uppercase)
                && v.split_whitespace().count() <= 5
                && !TU_THUONG.contains(&v)
            {
                ten.insert(v.to_string());
            }
        }
        let dai = ten
            .iter()
            .map(|t| t.split_whitespace().count())
            .max()
            .unwrap_or(1);
        Self { ten, dai }
    }

    pub fn la_ten(&self, s: &str) -> bool {
        self.ten.contains(s.trim())
    }

    /// Các tên riêng xuất hiện trong văn bản (cụm chữ viết hoa dài nhất).
    pub fn trong(&self, van_ban: &str) -> HashSet<String> {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = R.get_or_init(|| Regex::new(r"[A-Za-z][A-Za-z'’-]*").unwrap());
        let tu: Vec<&str> = r.find_iter(van_ban).map(|m| m.as_str()).collect();
        let mut kq = HashSet::new();
        for i in 0..tu.len() {
            if !tu[i].chars().next().is_some_and(char::is_uppercase) {
                continue;
            }
            for n in (1..=self.dai.min(tu.len() - i)).rev() {
                let cum = tu[i..i + n].join(" ");
                if self.ten.contains(&cum) {
                    kq.insert(cum);
                    break;
                }
            }
        }
        kq
    }
}

/// Lấy mảng JSON chuỗi từ câu trả lời (bỏ ```), đúng số phần tử.
pub fn tach_json(chu: &str, so: usize) -> Result<Vec<String>> {
    let chu = chu.trim();
    let (i, j) = (chu.find('['), chu.rfind(']'));
    let (Some(i), Some(j)) = (i, j) else {
        return Err(anyhow!("không thấy mảng JSON"));
    };
    if j < i {
        return Err(anyhow!("không thấy mảng JSON"));
    }
    let ds: Vec<String> = serde_json::from_str(&chu[i..=j])?;
    if ds.len() != so {
        return Err(anyhow!("sai số phần tử ({} / {so})", ds.len()));
    }
    Ok(ds)
}

/// Chia danh sách thành lô ≤ 40 câu, ≤ 6000 ký tự.
pub fn chia_lo(ds: &[String]) -> Vec<Vec<String>> {
    chia_lo_cap(ds, CAU_MOI_LO)
}

/// Như `chia_lo` nhưng tối đa `cau_moi_lo` câu một lô (vòng thử lại dùng lô nhỏ cho chắc).
pub fn chia_lo_cap(ds: &[String], cau_moi_lo: usize) -> Vec<Vec<String>> {
    let mut ra = Vec::new();
    let mut lo: Vec<String> = Vec::new();
    let mut ky = 0;
    for v in ds {
        let n = v.chars().count();
        if !lo.is_empty() && (lo.len() >= cau_moi_lo || ky + n > KY_TU_MOI_LO) {
            ra.push(std::mem::take(&mut lo));
            ky = 0;
        }
        lo.push(v.clone());
        ky += n;
    }
    if !lo.is_empty() {
        ra.push(lo);
    }
    ra
}

/// Giá USD / 1 triệu token từ cấu hình (gia_usd_moi_trieu_token), không có thì mặc định.
pub fn doc_gia() -> [f64; 2] {
    let v: serde_json::Value = std::fs::read(duong_dan::file_du_lieu("cau_hinh.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    match v["gia_usd_moi_trieu_token"].as_array() {
        Some(a) if a.len() == 2 => [
            a[0].as_f64().unwrap_or(GIA_MAC_DINH[0]),
            a[1].as_f64().unwrap_or(GIA_MAC_DINH[1]),
        ],
        _ => GIA_MAC_DINH,
    }
}

// ---------------- dịch offline giữ thẻ (giống class Offline của app cũ) ----------------

fn re_tach_cau() -> &'static Regex {
    // tách câu sau . ! ? … khi chữ sau bắt đầu bằng hoa / số / ngoặc / nháy
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"([.!?…])\s+(["'“A-Z0-9(])"#).unwrap())
}

fn tach_cau(s: &str) -> Vec<String> {
    let danh_dau = re_tach_cau().replace_all(s.trim(), "$1\u{1}$2");
    danh_dau
        .split('\u{1}')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(String::from)
        .collect()
}

/// Dịch các đoạn chữ (không có <..>, xuống dòng). Mã {..} thay bằng ZQn rồi trả lại; mô hình
/// làm mất mã thì dịch từng mảnh giữa các mã. `dich_cau` dịch một danh sách câu.
fn dich_doan(
    cac_doan: &[String],
    dich_cau: &dyn Fn(&[String]) -> Result<Vec<String>>,
) -> Result<Vec<String>> {
    static MA: OnceLock<Regex> = OnceLock::new();
    let re_ma = MA.get_or_init(|| Regex::new(r"\{[^{}]*\}").unwrap());
    let mut cau: Vec<String> = Vec::new();
    let mut khuon: Vec<(String, Vec<String>, usize, usize)> = Vec::new();
    for doan in cac_doan {
        let ma: Vec<String> = re_ma
            .find_iter(doan)
            .map(|m| m.as_str().to_string())
            .collect();
        let mut tam = doan.clone();
        for (n, m) in ma.iter().enumerate() {
            tam = tam.replacen(m.as_str(), &format!("ZQ{n}"), 1);
        }
        let ds = tach_cau(&tam);
        khuon.push((doan.clone(), ma, cau.len(), ds.len()));
        cau.extend(ds);
    }
    let ban = if cau.is_empty() {
        Vec::new()
    } else {
        dich_cau(&cau)?
    };
    let mut kq = Vec::with_capacity(khuon.len());
    for (doan, ma, dau, so) in &khuon {
        let mut vi = ban[*dau..dau + so].join(" ");
        if !ma.is_empty() {
            let du = (0..ma.len()).all(|n| vi.matches(&format!("ZQ{n}")).count() == 1)
                && !vi.contains(&format!("ZQ{}", ma.len()));
            if du {
                for (n, m) in ma.iter().enumerate() {
                    vi = vi.replace(&format!("ZQ{n}"), m);
                }
            } else {
                // dự phòng: dịch từng mảnh chữ giữa các mã
                let mut manh: Vec<String> = Vec::new();
                let mut cuoi = 0;
                for m in re_ma.find_iter(doan) {
                    manh.push(doan[cuoi..m.start()].to_string());
                    manh.push(m.as_str().to_string());
                    cuoi = m.end();
                }
                manh.push(doan[cuoi..].to_string());
                let can: Vec<String> = manh
                    .iter()
                    .filter(|m| {
                        !m.starts_with('{') && Regex::new(r"[A-Za-z]{2,}").unwrap().is_match(m)
                    })
                    .map(|m| m.trim().to_string())
                    .collect();
                let dich = if can.is_empty() {
                    Vec::new()
                } else {
                    dich_cau(&can)?
                };
                let mut it = dich.into_iter();
                vi = manh
                    .iter()
                    .map(|m| {
                        if !m.starts_with('{') && Regex::new(r"[A-Za-z]{2,}").unwrap().is_match(m) {
                            let trai = &m[..m.len() - m.trim_start().len()];
                            let phai = &m[m.trim_end().len()..];
                            format!("{trai}{}{phai}", it.next().unwrap_or_default())
                        } else {
                            m.clone()
                        }
                    })
                    .collect();
            }
        }
        kq.push(vi);
    }
    Ok(kq)
}

/// Dịch offline một lô chuỗi: giữ nguyên thẻ <..>, [EMPTY], xuống dòng; chỉ dịch phần chữ.
pub fn offline_lo(
    lo: &[String],
    dich_cau: &dyn Fn(&[String]) -> Result<Vec<String>>,
) -> Result<Vec<Option<String>>> {
    static R: OnceLock<Regex> = OnceLock::new();
    let re_tach = R.get_or_init(|| Regex::new(r"<[^<>]*>|\[EMPTY\]|\n").unwrap());
    enum Phan {
        Giu(String),
        Dich(String, usize, String),
    }
    let mut khuon: Vec<Vec<Phan>> = Vec::new();
    let mut can: Vec<String> = Vec::new();
    for s in lo {
        let mut phan = Vec::new();
        let mut cuoi = 0;
        let mut manh: Vec<String> = Vec::new();
        for m in re_tach.find_iter(s) {
            manh.push(s[cuoi..m.start()].to_string());
            manh.push(m.as_str().to_string());
            cuoi = m.end();
        }
        manh.push(s[cuoi..].to_string());
        for doan in manh.into_iter().filter(|d| !d.is_empty()) {
            let chi_the =
                re_tach.is_match(&doan) && re_tach.find(&doan).is_some_and(|m| m.as_str() == doan);
            if chi_the || khong_can_dich(&doan) {
                phan.push(Phan::Giu(doan));
                continue;
            }
            let trai = doan[..doan.len() - doan.trim_start().len()].to_string();
            let phai = doan[doan.trim_end().len()..].to_string();
            phan.push(Phan::Dich(trai, can.len(), phai));
            can.push(doan.trim().to_string());
        }
        khuon.push(phan);
    }
    let ban = if can.is_empty() {
        Vec::new()
    } else {
        dich_doan(&can, dich_cau)?
    };
    Ok(khuon
        .into_iter()
        .map(|phan| {
            let vi: String = phan
                .iter()
                .map(|p| match p {
                    Phan::Giu(s) => s.clone(),
                    Phan::Dich(t, i, ph) => format!("{t}{}{ph}", ban[*i]),
                })
                .collect();
            (!vi.trim().is_empty()).then_some(vi)
        })
        .collect())
}

// ---------------- việc dịch ----------------

/// Báo tình hình cho giao diện: (dòng thông báo) và (đã xong, tổng, tiền USD).
pub trait BaoCao: Sync {
    fn bao(&self, s: &str);
    fn tien_do(&self, xong: usize, tong: usize, usd: f64);
}

pub struct ViecDich {
    /// file bộ nhớ {tiếng Anh: tiếng Việt}
    pub duong_bo_nho: PathBuf,
    /// file đo chi phí thật (ước tính lần sau)
    pub duong_do_luong: PathBuf,
    pub huong_dan: String,
    pub bang_ten: BangTen,
    /// cách lấy thẻ để kiểm bản dịch (mặc định the_cua)
    pub the: fn(&str) -> Vec<String>,
    pub bo_nho: Arc<Mutex<BTreeMap<String, String>>>,
    pub dung: Arc<AtomicBool>,
    pub da_doi: usize,
    pub tien: f64,
    pub loi: usize,
}

impl ViecDich {
    /// `thu_muc`: thư mục dữ liệu của game (vd du_lieu\viet_hoa_game\crimson_desert).
    pub fn moi(thu_muc: PathBuf, huong_dan: &str, bang_ten: BangTen) -> Self {
        let duong_bo_nho = thu_muc.join("bo_nho_dich.json");
        let bo_nho: BTreeMap<String, String> = std::fs::read(&duong_bo_nho)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self {
            duong_do_luong: thu_muc.join("do_luong_chi_phi.json"),
            duong_bo_nho,
            huong_dan: huong_dan.to_string(),
            bang_ten,
            the: the_cua,
            bo_nho: Arc::new(Mutex::new(bo_nho)),
            dung: Arc::new(AtomicBool::new(false)),
            da_doi: 0,
            tien: 0.0,
            loi: 0,
        }
    }

    pub fn luu(&self) -> std::io::Result<()> {
        let b = serde_json::to_vec(&*self.bo_nho.lock().unwrap_or_else(|e| e.into_inner()))?;
        duong_dan::ghi_file(&self.duong_bo_nho, &b)
    }

    pub fn tim(&self, en: &str) -> Option<String> {
        self.bo_nho
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(en)
            .cloned()
    }

    /// Các chuỗi (không trùng, giữ thứ tự) chưa có trong bộ nhớ.
    pub fn cau_can_dich<'a>(&self, ds: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let bn = self.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
        let mut da = HashSet::new();
        ds.into_iter()
            .filter(|v| !bn.contains_key(*v) && da.insert(*v))
            .map(String::from)
            .collect()
    }

    /// Ước tính USD theo số đo thật các lần trước; chưa có thì dùng hệ số đã hiệu chỉnh (×4,2).
    pub fn uoc_tinh_usd(&self, ky_tu: usize) -> f64 {
        let d: serde_json::Value = std::fs::read(&self.duong_do_luong)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        if let (Some(k), Some(u)) = (d["ky_tu"].as_f64(), d["usd"].as_f64()) {
            if k > 50_000.0 {
                return ky_tu as f64 * u / k;
            }
        }
        let gia = doc_gia();
        let k = ky_tu as f64;
        let vao = k / 3.6 * 1.35 + 900.0 * (k / KY_TU_MOI_LO as f64 + 1.0);
        let ra = k / 3.6 * 1.9;
        (vao * gia[0] / 1e6 + ra * gia[1] / 1e6) * 4.2
    }

    fn ghi_do_luong(&self, ky_tu: usize, usd: f64) {
        if ky_tu == 0 || usd <= 0.0 {
            return;
        }
        let mut d: serde_json::Value = std::fs::read(&self.duong_do_luong)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_else(|| serde_json::json!({"ky_tu": 0, "usd": 0.0}));
        d["ky_tu"] = (d["ky_tu"].as_u64().unwrap_or(0) + ky_tu as u64).into();
        d["usd"] = (d["usd"].as_f64().unwrap_or(0.0) + usd).into();
        let _ = duong_dan::ghi_file(&self.duong_do_luong, d.to_string().as_bytes());
    }

    /// Dịch một lô bằng Claude; trả (bản dịch | None nếu hỏng, tiền).
    fn dich_lo_claude(&self, lo: &[String], gia: [f64; 2]) -> Result<(Vec<Option<String>>, f64)> {
        let mut ten: Vec<String> = lo
            .iter()
            .flat_map(|v| self.bang_ten.trong(v))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        ten.sort();
        let mut noi_dung = String::new();
        if !ten.is_empty() {
            noi_dung.push_str(&format!(
                "Giữ nguyên (không dịch): {}\n\n",
                ten.into_iter().take(150).collect::<Vec<_>>().join(", ")
            ));
        }
        if lo.len() <= 8 {
            // lô nhỏ (vòng thử lại): nhắc rõ các thẻ phải giữ để khỏi bị loại vì sai thẻ
            let mut the_dong: Vec<String> = lo.iter().flat_map(|v| (self.the)(v)).collect();
            the_dong.sort();
            the_dong.dedup();
            if !the_dong.is_empty() {
                noi_dung.push_str(&format!(
                    "Mỗi bản dịch PHẢI chứa đúng các thẻ của câu gốc, giữ nguyên từng ký tự: {}\n\n",
                    the_dong.join("  ")
                ));
            }
        }
        noi_dung.push_str(&serde_json::to_string(lo)?);
        let ky: usize = lo.iter().map(|v| v.chars().count()).sum();
        let toi_da = ((ky as f64 / 1.2) as u32 + 800).min(16000);
        let mut tien = 0.0;
        let mut kq = None;
        for _ in 0..2 {
            let (chu, vao, ra) = claude::goi_day_du(&self.huong_dan, &noi_dung, toi_da)?;
            tien += vao as f64 * gia[0] / 1e6 + ra as f64 * gia[1] / 1e6;
            if let Ok(v) = tach_json(&chu, lo.len()) {
                kq = Some(v);
                break;
            }
        }
        let Some(kq) = kq else {
            if lo.len() > 1 {
                // chia đôi rồi thử lại
                let giua = lo.len() / 2;
                let (mut a, ta) = self.dich_lo_claude(&lo[..giua], gia)?;
                let (b, tb) = self.dich_lo_claude(&lo[giua..], gia)?;
                a.extend(b);
                return Ok((a, tien + ta + tb));
            }
            return Ok((vec![None], tien));
        };
        Ok((
            lo.iter()
                .zip(kq)
                .map(|(en, vi)| {
                    (!vi.trim().is_empty() && (self.the)(en) == (self.the)(&vi)).then_some(vi)
                })
                .collect(),
            tien,
        ))
    }

    /// Một vòng dịch Claude 4 luồng trên `ds`. Trả (tiền vòng này, lỗi dừng nếu có).
    fn vong_claude(
        &mut self,
        ds: &[String],
        gioi_han_usd: f64,
        tien_truoc: f64,
        bc: &dyn BaoCao,
        cau_moi_lo: usize,
        ten_vong: &str,
    ) -> Result<(f64, Option<String>)> {
        let tong = ds.len();
        let gia = doc_gia();
        let cac_lo = chia_lo_cap(ds, cau_moi_lo);
        bc.bao(&format!("{ten_vong}: {tong} câu ({} lô)...", cac_lo.len()));
        let hang = Mutex::new(cac_lo.into_iter());
        let chung = Mutex::new((
            0usize,
            0.0f64,
            0usize,
            0usize,
            Instant::now(),
            None::<String>,
        )); // xong, tiền, đổi, lỗi, lần lưu, lỗi dừng
        let viec: &ViecDich = self;
        std::thread::scope(|s| {
            for _ in 0..SO_LUONG {
                s.spawn(|| loop {
                    if viec.dung.load(Ordering::SeqCst) || tien_truoc + chung.lock().unwrap().1 >= gioi_han_usd {
                        return;
                    }
                    let Some(lo) = hang.lock().unwrap().next() else {
                        return;
                    };
                    match viec.dich_lo_claude(&lo, gia) {
                        Ok((kq, tien)) => {
                            let mut doi = 0;
                            let mut hong = 0;
                            {
                                let mut bn = viec.bo_nho.lock().unwrap();
                                for (en, vi) in lo.iter().zip(kq) {
                                    match vi {
                                        Some(vi) => {
                                            bn.insert(en.clone(), crate::dich::ap_dung_sua(&vi));
                                            doi += 1;
                                        }
                                        None => hong += 1,
                                    }
                                }
                            }
                            viec.ghi_do_luong(lo.iter().map(|v| v.chars().count()).sum(), tien);
                            let mut c = chung.lock().unwrap();
                            c.0 += lo.len();
                            c.1 += tien;
                            c.2 += doi;
                            c.3 += hong;
                            bc.tien_do(c.0, tong, c.1);
                            if c.4.elapsed().as_secs() > 20 {
                                c.4 = Instant::now();
                                let _ = viec.luu();
                            }
                        }
                        Err(e) if e.downcast_ref::<claude::LoiDung>().is_some() => {
                            viec.dung.store(true, Ordering::SeqCst);
                            chung.lock().unwrap().5 = Some(e.to_string());
                            return;
                        }
                        Err(e) => {
                            chung.lock().unwrap().3 += lo.len();
                            bc.bao(&format!(
                                "Lỗi một lô ({e}) – bỏ qua, lần sau chạy tiếp sẽ dịch lại"
                            ));
                        }
                    }
                });
            }
        });
        let (_, tien, doi, _loi, _, loi_dung) = chung.into_inner().unwrap();
        self.da_doi += doi;
        self.luu()?;
        Ok((tien, loi_dung))
    }

    /// Dịch bằng Claude (4 luồng) tới khi hết câu / chạm giới hạn tiền / bị dừng.
    /// Câu nào hỏng (sai thẻ, sai số phần tử...) được thử lại tối đa 2 vòng với lô nhỏ hơn.
    /// Trả true nếu đã dịch xong hết. `self.loi` = số câu còn chưa dịch được.
    pub fn chay_claude(
        &mut self,
        ds: Vec<String>,
        gioi_han_usd: f64,
        bc: &dyn BaoCao,
    ) -> Result<bool> {
        if ds.is_empty() {
            bc.bao("Đã dịch xong hết.");
            return Ok(true);
        }
        let _thuc = duong_dan::GiuMayThuc::moi();
        self.dung.store(false, Ordering::SeqCst);
        self.tien = 0.0;
        self.loi = 0;
        let mut can = ds.clone();
        for (vong, cau_lo, ten) in [(0, CAU_MOI_LO, "Dịch"), (1, 8, "Thử lại câu hỏng"), (2, 1, "Thử lại từng câu")] {
            if can.is_empty() {
                break;
            }
            let (tien, loi_dung) = self.vong_claude(&can, gioi_han_usd, self.tien, bc, cau_lo, ten)?;
            self.tien += tien;
            if let Some(e) = loi_dung {
                bc.bao(&format!("Dừng: {e}"));
                self.loi = self.cau_can_dich(ds.iter().map(String::as_str)).len();
                return Ok(false);
            }
            if self.dung.load(Ordering::SeqCst) {
                break;
            }
            if self.tien >= gioi_han_usd {
                bc.bao(&format!(
                    "Đã chạm giới hạn {gioi_han_usd:.2} USD – bấm Dịch lần nữa để chạy tiếp."
                ));
                break;
            }
            can = self.cau_can_dich(ds.iter().map(String::as_str));
            if vong == 0 && !can.is_empty() {
                duong_dan::nhat_ky(&format!("Claude: {} câu chưa qua kiểm thẻ, thử lại lô nhỏ", can.len()));
            }
        }
        let con = self.cau_can_dich(ds.iter().map(String::as_str));
        self.loi = con.len();
        if !con.is_empty() {
            self.ghi_cau_chua_dich(&con);
        }
        Ok(!self.dung.load(Ordering::SeqCst) && con.is_empty())
    }

    /// Ghi các câu chưa dịch được ra cau_chua_dich.txt cạnh bộ nhớ để xem / dịch tay.
    fn ghi_cau_chua_dich(&self, con: &[String]) {
        if let Some(thu_muc) = self.duong_bo_nho.parent() {
            let nd: String = con.iter().map(|c| format!("{}\n", c.replace('\n', "\\n"))).collect();
            let _ = duong_dan::ghi_file(&thu_muc.join("cau_chua_dich.txt"), nd.as_bytes());
        }
        for c in con.iter().take(5) {
            duong_dan::nhat_ky(&format!("Chưa dịch được: {}", c.chars().take(120).collect::<String>()));
        }
    }

    /// Dịch offline (CHỈ tiếng Anh) theo lô 24 câu. Trả true nếu xong hết.
    pub fn chay_offline(&mut self, ds: Vec<String>, bc: &dyn BaoCao) -> Result<bool> {
        if !offline::co_the_dung() {
            return Err(claude::LoiDung(
                "chưa có Python riêng / mô hình dịch offline (Cài đặt > Tải công cụ)".into(),
            )
            .into());
        }
        let tong = ds.len();
        if tong == 0 {
            bc.bao("Đã dịch xong hết.");
            return Ok(true);
        }
        let _thuc = duong_dan::GiuMayThuc::moi();
        self.dung.store(false, Ordering::SeqCst);
        self.loi = 0;
        let mut hong: Vec<String> = Vec::new();
        bc.bao(&format!("Bắt đầu dịch offline {tong} câu..."));
        // dịch từng câu: chữ IN HOA hạ thường trước, từ menu dịch thẳng, cắt bản dịch lặp
        let dich_cau = |cau: &[String]| -> Result<Vec<String>> {
            let chuan: Vec<(String, bool)> = cau.iter().map(|c| crate::dich::ha_hoa(c)).collect();
            let vao: Vec<String> = chuan.iter().map(|(c, _)| c.clone()).collect();
            let mut ra = Vec::with_capacity(cau.len());
            for phan in vao.chunks(32) {
                ra.extend(offline::dich_lo(phan)?);
            }
            Ok(ra
                .into_iter()
                .zip(chuan)
                .zip(cau)
                .map(|((vi, (_, hoa)), en)| {
                    let vi = if hoa { vi.to_uppercase() } else { vi };
                    crate::dich::the::tu_menu(en)
                        .unwrap_or_else(|| crate::dich::the::bo_lap_doi(en, &vi))
                })
                .collect())
        };
        let mut xong = 0;
        let mut lan_luu = Instant::now();
        for lo in ds.chunks(24) {
            if self.dung.load(Ordering::SeqCst) {
                break;
            }
            let kq = offline_lo(lo, &dich_cau).unwrap_or_else(|e| {
                bc.bao(&format!("Lỗi dịch offline ({e}) – bỏ qua lô này"));
                vec![None; lo.len()]
            });
            {
                let mut bn = self.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
                for (en, vi) in lo.iter().zip(kq) {
                    match vi {
                        Some(vi)
                            if (self.the)(en) == (self.the)(&vi)
                                && !crate::dich::the::bi_lap(en, &vi) =>
                        {
                            bn.insert(en.clone(), crate::dich::ap_dung_sua(&vi));
                            self.da_doi += 1;
                        }
                        _ => hong.push(en.clone()),
                    }
                }
            }
            xong += lo.len();
            bc.tien_do(xong, tong, 0.0);
            if lan_luu.elapsed().as_secs() > 30 {
                self.luu()?;
                lan_luu = Instant::now();
            }
        }
        // Câu bị loại (sai thẻ, dịch lặp...): thử lại bằng Google (vẫn giữ thẻ qua offline_lo)
        if !hong.is_empty() && !self.dung.load(Ordering::SeqCst) {
            bc.bao(&format!("{} câu offline dịch hỏng – thử lại bằng Google...", hong.len()));
            let dich_google = |cau: &[String]| -> Result<Vec<String>> {
                let (ra, loi) = crate::dich::google::dich_nhieu(cau);
                if ra.iter().any(Option::is_none) {
                    return Err(anyhow!("Google lỗi: {}", loi.unwrap_or_default()));
                }
                Ok(ra.into_iter().zip(cau).map(|(v, en)| crate::dich::the::bo_lap_doi(en, &v.unwrap_or_default())).collect())
            };
            for lo in hong.chunks(24) {
                if self.dung.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(kq) = offline_lo(lo, &dich_google) else { continue };
                let mut bn = self.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
                for (en, vi) in lo.iter().zip(kq) {
                    if let Some(vi) = vi {
                        if (self.the)(en) == (self.the)(&vi) && !crate::dich::the::bi_lap(en, &vi) {
                            bn.insert(en.clone(), crate::dich::ap_dung_sua(&vi));
                            self.da_doi += 1;
                        }
                    }
                }
            }
        }
        // vẫn còn câu hỏng: nhờ AI local (nếu Ollama đang chạy và không đang lồng tiếng)
        let con_hong: Vec<String> = self.cau_can_dich(ds.iter().map(String::as_str)).into_iter().map(|s| s.to_string()).collect();
        if !con_hong.is_empty() && !self.dung.load(Ordering::SeqCst) && crate::ai_local::san_sang() {
            bc.bao(&format!("{} câu còn lại – thử bằng AI local...", con_hong.len()));
            let dich_ai = |cau: &[String]| -> Result<Vec<String>> {
                let ra = crate::ai_local::dich_lo(cau)?;
                Ok(ra.into_iter().zip(cau).map(|(v, en)| crate::dich::the::bo_lap_doi(en, &v)).collect())
            };
            for lo in con_hong.chunks(12) {
                if self.dung.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(kq) = offline_lo(lo, &dich_ai) else { continue };
                let mut bn = self.bo_nho.lock().unwrap_or_else(|e| e.into_inner());
                for (en, vi) in lo.iter().zip(kq) {
                    if let Some(vi) = vi {
                        if (self.the)(en) == (self.the)(&vi) && !crate::dich::the::bi_lap(en, &vi) {
                            bn.insert(en.clone(), crate::dich::ap_dung_sua(&vi));
                            self.da_doi += 1;
                        }
                    }
                }
            }
        }
        self.luu()?;
        let con = self.cau_can_dich(ds.iter().map(String::as_str));
        self.loi = con.len();
        if !con.is_empty() && !self.dung.load(Ordering::SeqCst) {
            self.ghi_cau_chua_dich(&con);
        }
        Ok(!self.dung.load(Ordering::SeqCst) && con.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loc_cau() {
        assert!(khong_can_dich("<br/>{Param1} 123"));
        assert!(!khong_can_dich("Open the {Key:door}"));
        assert!(la_cau("You must find the second shrine before nightfall."));
        assert!(!la_cau("Iron Sword"));
        assert_eq!(
            the_cua("a {Param1} <br/> [EMPTY]"),
            vec!["<br/>", "[EMPTY]", "{Param1}"]
        );
    }

    #[test]
    fn bang_ten_va_lo() {
        let bt = BangTen::tao([
            "Kliff",
            "Greymanes",
            "Pywel",
            "Talk",
            "A very long sentence here.",
        ]);
        assert!(bt.la_ten("Kliff") && !bt.la_ten("Talk"));
        let t = bt.trong("Kliff leads the Greymanes across Pywel.");
        assert_eq!(t.len(), 3);
        let ds: Vec<String> = (0..100).map(|i| format!("câu {i}")).collect();
        let lo = chia_lo(&ds);
        assert_eq!(lo.len(), 3);
        assert_eq!(lo[0].len(), 40);
        assert_eq!(
            tach_json("```json\n[\"a\",\"b\"]\n```", 2).unwrap(),
            vec!["a", "b"]
        );
        assert!(tach_json("[\"a\"]", 2).is_err());
    }

    #[test]
    fn offline_giu_the() {
        // "máy dịch" giả: thêm [vi] trước mỗi câu, giữ ZQn
        let gia = |cau: &[String]| -> Result<Vec<String>> {
            Ok(cau.iter().map(|c| format!("[vi]{c}")).collect())
        };
        let kq = offline_lo(
            &[
                "Hello there.<br/>Press {Key:Jump} now!".to_string(),
                "<b>Gold</b>\nSilver".to_string(),
                "123".to_string(),
            ],
            &gia,
        )
        .unwrap();
        assert_eq!(
            kq[0].as_deref(),
            Some("[vi]Hello there.<br/>[vi]Press {Key:Jump} now!")
        );
        assert_eq!(kq[1].as_deref(), Some("<b>[vi]Gold</b>\n[vi]Silver"));
        assert_eq!(kq[2].as_deref(), Some("123"));
        assert_eq!(tach_cau("One. Two! three"), vec!["One.", "Two! three"]);
    }

    /// Thử thật với mô hình offline (chạy riêng: cargo test -- --ignored offline_that)
    #[test]
    #[ignore]
    fn offline_that() {
        struct In;
        impl BaoCao for In {
            fn bao(&self, s: &str) {
                println!("{s}");
            }
            fn tien_do(&self, xong: usize, tong: usize, _: f64) {
                println!("{xong}/{tong}");
            }
        }
        let thu_muc = std::env::temp_dir().join("viet_hoa_thu_offline");
        let _ = std::fs::remove_dir_all(&thu_muc);
        std::fs::create_dir_all(&thu_muc).unwrap();
        let mut v = ViecDich::moi(thu_muc.clone(), "", BangTen::tao(["Kliff"]));
        let ds: Vec<String> = [
            "Press {Key:Key_Interact} to open the chest.",
            "Kliff must find the second shrine.<br/>Hurry before nightfall!",
            "<b>Quest Complete</b>",
            "You received {Param1} gold.",
            "The village is under attack.\nWe need to leave now!",
            "Settings",
            "123",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let can = v.cau_can_dich(ds.iter().map(String::as_str));
        assert_eq!(can.len(), 7);
        let xong = v.chay_offline(can, &In).unwrap();
        for en in &ds {
            println!("{en}  =>  {:?}", v.tim(en));
        }
        assert!(xong, "phải dịch xong hết");
        for en in &ds {
            let vi = v.tim(en).unwrap();
            assert_eq!(the_cua(en), the_cua(&vi), "giữ thẻ: {en} => {vi}");
        }
        // lần 2: không còn câu cần dịch (đã lưu bộ nhớ)
        let v2 = ViecDich::moi(thu_muc, "", BangTen::default());
        assert!(v2.cau_can_dich(ds.iter().map(String::as_str)).is_empty());
    }
}
