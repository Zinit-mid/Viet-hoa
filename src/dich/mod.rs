//! Máy dịch dùng chung (giống BoDich của app cũ):
//! bản Việt hóa game → bản Claude đã lưu → Offline → Google; Claude dịch ngầm để lưu.

pub mod bo_nho;
pub mod claude;
pub mod google;
pub mod offline;
pub mod the;

use std::sync::Mutex;

use bo_nho::{BoNhoDich, BoNhoVietHoa};

static BO_NHO: Mutex<Option<BoNhoDich>> = Mutex::new(None);
static VIET_HOA: Mutex<Option<BoNhoVietHoa>> = Mutex::new(None);
static SUA: Mutex<Option<Vec<(String, String)>>> = Mutex::new(None);

fn bo_nho<R>(f: impl FnOnce(&mut BoNhoDich) -> R) -> R {
    let mut k = BO_NHO.lock().unwrap_or_else(|e| e.into_inner());
    f(k.get_or_insert_with(|| BoNhoDich::nap("ban_dich_da_luu.json")))
}

/// Nạp sẵn bộ nhớ dịch (gộp bộ nhớ app cũ) lúc mở app. Trả số mục.
pub fn nap_san() -> usize {
    bo_nho(|b| b.so_cau())
}

pub fn luu_bo_nho() {
    bo_nho(|b| b.luu());
}

pub fn luu(may: &str, en: &str, vi: &str) {
    bo_nho(|b| b.them(may, en, vi));
}

/// Sửa bản dịch theo tu_dien.json ("sua_ban_dich": {sai: đúng}) như app cũ.
fn sua(s: String) -> String {
    let mut k = SUA.lock().unwrap_or_else(|e| e.into_inner());
    let ds = k.get_or_insert_with(|| {
        let v: serde_json::Value = std::fs::read(crate::duong_dan::file_du_lieu("tu_dien.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        v.get("sua_ban_dich")
            .and_then(|m| m.as_object())
            .map(|m| m.iter().filter_map(|(a, b)| Some((a.clone(), b.as_str()?.to_string()))).collect())
            .unwrap_or_default()
    });
    ds.iter().fold(s, |s, (sai, dung)| s.replace(sai.as_str(), dung))
}

/// Bản Việt hóa game (Crimson Desert...): giống hệt chữ trong game, miễn phí.
pub fn tim_viet_hoa(chu: &str) -> Option<String> {
    let mut k = VIET_HOA.lock().unwrap_or_else(|e| e.into_inner());
    k.get_or_insert_with(BoNhoVietHoa::default).tim(chu).map(sua)
}

/// Bản đã lưu: Việt hóa game → Claude → (Claude không dùng được thì) Google.
pub fn tim_da_luu(chu: &str) -> Option<String> {
    if let Some(v) = tim_viet_hoa(chu) {
        return Some(v);
    }
    if let Some(v) = bo_nho(|b| b.tim("claude", chu)) {
        return Some(sua(v));
    }
    if !claude::co_khoa() {
        return bo_nho(|b| b.tim("google", chu)).map(sua);
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum May {
    VietHoa,
    DaLuu,
    Offline,
    Claude,
    Google,
}

impl May {
    pub fn ten(self) -> &'static str {
        match self {
            May::VietHoa => "bản Việt hóa game",
            May::DaLuu => "bản đã lưu",
            May::Offline => "offline",
            May::Claude => "Claude",
            May::Google => "Google",
        }
    }
}

/// Dịch nhanh một câu (hội thoại) theo chế độ – giống _dich_nhanh của app cũ.
/// `che_do`: offline | offline_claude | claude | google.
pub fn dich_nhanh(chu: &str, che_do: &str) -> Option<(String, May)> {
    let chu = chu.trim();
    if chu.is_empty() {
        return None;
    }
    if let Some(v) = tim_viet_hoa(chu) {
        return Some((v, May::VietHoa));
    }
    if che_do != "google" {
        if let Some(v) = bo_nho(|b| b.tim("claude", chu)) {
            return Some((sua(v), May::DaLuu));
        }
    }
    if let Some(v) = the::tu_menu(chu) {
        return Some((v, May::DaLuu));
    }
    if che_do == "claude" && claude::co_khoa() {
        match claude::dich(chu) {
            Ok(v) if hop_le(chu, &v) => {
                luu("claude", chu, &v);
                return Some((sua(v), May::Claude));
            }
            Ok(_) => {}
            Err(e) => ghi_loi(&format!("Máy dịch Claude lỗi: {e}")),
        }
    }
    if che_do.starts_with("offline") && offline::co_the_dung() {
        if let Some(v) = offline_lo(&[chu.to_string()]).and_then(|mut v| v.pop().flatten()) {
            return Some((sua(v), May::Offline));
        }
    }
    if let Some(v) = bo_nho(|b| b.tim("google", chu)) {
        return Some((sua(v), May::DaLuu));
    }
    let v = google_lo(&[chu.to_string()]).pop().flatten()?;
    Some((sua(v), May::Google))
}

/// Claude dịch ngầm để lưu (chế độ offline_claude). Trả bản Claude nếu có.
pub fn claude_luu_ngam(chu: &str) -> Option<String> {
    if !claude::co_khoa() || bo_nho(|b| b.tim("claude", chu)).is_some() {
        return None;
    }
    match claude::dich(chu) {
        Ok(v) if hop_le(chu, &v) && !the::con_chu_anh(&v) => {
            luu("claude", chu, &v);
            Some(sua(v))
        }
        Ok(_) => None,
        Err(e) => {
            ghi_loi(&format!("Máy dịch Claude lỗi: {e}"));
            None
        }
    }
}

/// Ghi lỗi vào nhật ký; cùng một lỗi chỉ ghi lại sau 10 giây.
fn ghi_loi(loi: &str) {
    static CUOI: Mutex<Option<(String, std::time::Instant)>> = Mutex::new(None);
    if let Ok(mut c) = CUOI.lock() {
        let lap = c.as_ref().is_some_and(|(l, t)| l == loi && t.elapsed().as_secs() < 10);
        if !lap {
            crate::duong_dan::nhat_ky(loi);
            *c = Some((loi.to_string(), std::time::Instant::now()));
        }
    }
}

/// Bản dịch dùng được: không rỗng, đúng thẻ, không lặp từ vô cớ.
pub fn hop_le(en: &str, vi: &str) -> bool {
    !vi.trim().is_empty() && the::khop_the(en, vi) && !the::bi_lap(en, vi)
}

/// Chữ toàn IN HOA → câu thường ("WINDOW SIZE" → "Window size"). Trả (câu, có IN HOA).
pub fn ha_hoa(s: &str) -> (String, bool) {
    let chu: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
    if chu.len() < 2 || !chu.iter().all(|c| c.is_uppercase()) {
        return (s.to_string(), false);
    }
    let thuong = s.to_lowercase();
    let mut it = thuong.chars();
    let ra = match it.next() {
        Some(f) => f.to_uppercase().collect::<String>() + it.as_str(),
        None => String::new(),
    };
    (ra, true)
}

/// Tách "Tên: lời thoại" (tên tối đa 3 từ viết hoa đầu) → (tên, lời).
fn tach_ten(dong: &str) -> (&str, &str) {
    if let Some((ten, loi)) = dong.split_once(": ") {
        let tu: Vec<&str> = ten.split(' ').collect();
        let la_ten = !tu.is_empty()
            && tu.len() <= 3
            && tu.iter().all(|w| {
                w.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    && w.chars().all(|c| c.is_ascii_alphabetic() || "'.-".contains(c))
            });
        if la_ten && !loi.trim().is_empty() {
            return (ten, loi.trim());
        }
    }
    ("", dong)
}

/// Dịch offline (giống DichOffline.dich_nhieu của app cũ): dịch từng dòng, giữ tên
/// người nói; xử lý chữ IN HOA, cắt lặp, bỏ bản hỏng (None).
fn offline_lo(cau: &[String]) -> Option<Vec<Option<String>>> {
    struct Dong {
        cau: usize,
        ten: String,
        hoa: bool,
        goc: String,
    }
    let mut dong: Vec<Dong> = Vec::new();
    for (i, c) in cau.iter().enumerate() {
        for d in c.lines().map(str::trim).filter(|d| !d.is_empty()) {
            let (ten, loi) = tach_ten(d);
            let (_, hoa) = ha_hoa(loi);
            dong.push(Dong { cau: i, ten: ten.to_string(), hoa, goc: loi.to_string() });
        }
    }
    let vao: Vec<String> = dong.iter().map(|d| ha_hoa(&d.goc).0).collect();
    let ra = if vao.is_empty() {
        Vec::new()
    } else {
        match offline::dich_lo(&vao) {
            Ok(v) if v.len() == vao.len() => v,
            Ok(_) => return None,
            Err(e) => {
                ghi_loi(&format!("Máy dịch offline lỗi: {e}"));
                return None;
            }
        }
    };
    // Dòng dịch hỏng → giữ nguyên tiếng Anh dòng đó; cả đoạn hỏng hết mới trả None
    let mut ket: Vec<Vec<String>> = vec![Vec::new(); cau.len()];
    let mut tot = vec![0usize; cau.len()];
    for (d, vi) in dong.iter().zip(ra) {
        let vi = if d.hoa { vi.to_uppercase() } else { vi };
        let vi = the::tu_menu(&d.goc).unwrap_or_else(|| the::bo_lap_doi(&d.goc, &vi));
        let vi = if hop_le(&d.goc, &vi) {
            tot[d.cau] += 1;
            vi
        } else {
            crate::duong_dan::nhat_ky(&format!("Bỏ bản dịch offline hỏng: {} => {}", d.goc, vi.chars().take(80).collect::<String>()));
            d.goc.clone()
        };
        ket[d.cau].push(if d.ten.is_empty() { vi } else { format!("{}: {vi}", d.ten) });
    }
    Some(ket.into_iter().zip(tot).map(|(v, t)| (t > 0).then(|| v.join("\n"))).collect())
}

/// Google dịch cả lô (ít lần gọi), bỏ bản hỏng, lưu "google".
pub fn google_lo(cau: &[String]) -> Vec<Option<String>> {
    let (ra, loi) = google::dich_nhieu(cau);
    if let Some(e) = loi {
        ghi_loi(&format!("Máy dịch Google lỗi: {e}"));
    }
    ra.into_iter()
        .zip(cau)
        .map(|(vi, en)| {
            let vi = vi.map(|v| the::bo_lap_doi(en, &v)).filter(|v| hop_le(en, v))?;
            luu("google", en, &vi);
            Some(vi)
        })
        .collect()
}

/// Dịch nhiều câu một lúc (F12, Việt hóa file): đã lưu → Claude lô → Offline → Google.
pub fn dich_nhieu(cac_cau: &[String], dung_claude: bool) -> Vec<Option<String>> {
    let mut ra: Vec<Option<String>> = cac_cau
        .iter()
        .map(|c| tim_da_luu(c).or_else(|| the::tu_menu(c)).or_else(|| bo_nho(|b| b.tim("google", c)).map(sua)))
        .collect();
    let con_thieu = |ra: &Vec<Option<String>>| -> Vec<usize> { (0..cac_cau.len()).filter(|&i| ra[i].is_none()).collect() };

    let can = con_thieu(&ra);
    if !can.is_empty() && dung_claude && claude::co_khoa() {
        let cau: Vec<String> = can.iter().map(|&i| cac_cau[i].clone()).collect();
        match claude::dich_lo(&cau, claude::HUONG_DAN) {
            Ok(v) => {
                for (k, &i) in can.iter().enumerate() {
                    if hop_le(&cac_cau[i], &v[k]) {
                        luu("claude", &cac_cau[i], &v[k]);
                        ra[i] = Some(sua(v[k].clone()));
                    }
                }
            }
            Err(e) => ghi_loi(&format!("Máy dịch Claude lỗi: {e}")),
        }
    }
    let can = con_thieu(&ra);
    if !can.is_empty() && offline::co_the_dung() {
        let cau: Vec<String> = can.iter().map(|&i| cac_cau[i].clone()).collect();
        if let Some(v) = offline_lo(&cau) {
            for (k, &i) in can.iter().enumerate() {
                ra[i] = v[k].clone().map(sua);
            }
        }
    }
    let can = con_thieu(&ra);
    if !can.is_empty() {
        let cau: Vec<String> = can.iter().map(|&i| cac_cau[i].clone()).collect();
        for (k, vi) in google_lo(&cau).into_iter().enumerate() {
            ra[can[k]] = vi.map(sua);
        }
    }
    ra
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chu_in_hoa_va_ten() {
        assert_eq!(ha_hoa("WINDOW SIZE"), ("Window size".to_string(), true));
        assert_eq!(ha_hoa("Press E to open"), ("Press E to open".to_string(), false));
        assert_eq!(tach_ten("Kliff: Let's go."), ("Kliff", "Let's go."));
        assert_eq!(tach_ten("press: e"), ("", "press: e"));
    }
}
