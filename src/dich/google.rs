//! Google Translate (gtx) – miễn phí, cần mạng. Dùng dự phòng / hiện ngay.

use anyhow::{anyhow, Result};
use std::time::Duration;

/// Google báo "gọi quá nhiều" (429) → nghỉ gọi tới thời điểm này.
static CHAN_DEN: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

pub fn dang_bi_chan() -> bool {
    CHAN_DEN.lock().ok().and_then(|c| *c).is_some_and(|t| std::time::Instant::now() < t)
}

pub fn dich(chu: &str) -> Result<String> {
    if dang_bi_chan() {
        return Err(anyhow!("Google đang tạm chặn (gọi quá nhiều) – nghỉ 60 giây, dùng offline"));
    }
    let tra = ureq::get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", "vi")
        .query("dt", "t")
        .query("q", chu)
        .timeout(Duration::from_secs(8))
        .call();
    let tra: serde_json::Value = match tra {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(429, _)) => {
            if let Ok(mut c) = CHAN_DEN.lock() {
                *c = Some(std::time::Instant::now() + Duration::from_secs(60));
            }
            return Err(anyhow!("Google tạm chặn (429) – nghỉ 60 giây, dùng offline"));
        }
        Err(ureq::Error::Status(ma, _)) => return Err(anyhow!("Google lỗi {ma}")),
        Err(e) => return Err(anyhow!("Không kết nối được Google: {}", e.kind())),
    };
    let mut ra = String::new();
    for doan in tra.get(0).and_then(|v| v.as_array()).ok_or_else(|| anyhow!("Google trả sai dạng"))? {
        if let Some(s) = doan.get(0).and_then(|v| v.as_str()) {
            ra.push_str(s);
        }
    }
    if ra.is_empty() {
        return Err(anyhow!("Google trả rỗng"));
    }
    Ok(ra)
}

/// Dịch nhiều câu trong ít lần gọi: nối bằng xuống dòng (mỗi lần ≤ 1500 ký tự).
/// Trả (bản dịch từng câu, lỗi gặp phải nếu có).
pub fn dich_nhieu(cau: &[String]) -> (Vec<Option<String>>, Option<String>) {
    let mut ra: Vec<Option<String>> = vec![None; cau.len()];
    let mut loi = None;
    let mut dau = 0;
    while dau < cau.len() {
        let mut cuoi = dau;
        let mut dai = 0;
        while cuoi < cau.len() && (cuoi == dau || dai + cau[cuoi].len() < 1500) {
            dai += cau[cuoi].len() + 1;
            cuoi += 1;
        }
        let nhom: Vec<String> = cau[dau..cuoi].iter().map(|c| c.replace('\n', " ")).collect();
        match dich(&nhom.join("\n")) {
            Ok(vi) => {
                let dong: Vec<&str> = vi.split('\n').collect();
                if dong.len() == nhom.len() {
                    for (k, d) in dong.into_iter().enumerate() {
                        ra[dau + k] = Some(d.trim().to_string());
                    }
                } else {
                    // Google gộp / tách dòng khác đi → dịch từng câu
                    for k in dau..cuoi {
                        match dich(&cau[k]) {
                            Ok(v) => ra[k] = Some(v),
                            Err(e) => loi = Some(e.to_string()),
                        }
                    }
                }
            }
            Err(e) => {
                loi = Some(e.to_string());
                break; // mạng / Google chặn: dừng, lần quét sau thử lại
            }
        }
        dau = cuoi;
    }
    (ra, loi)
}
