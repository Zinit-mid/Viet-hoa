//! Claude API (claude-haiku-5-5). Mã API chỉ đọc từ du_lieu\khoa_api.txt.

use anyhow::{anyhow, bail, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::cau_hinh;

pub const MO_HINH: &str = "claude-haiku-5-5";

pub const HUONG_DAN: &str = "Bạn là người dịch game chuyên nghiệp từ tiếng Anh (hoặc ngôn ngữ khác) sang tiếng Việt.\n\
- Dịch tự nhiên, ngắn gọn, đúng giọng nhân vật trong game.\n\
- GIỮ NGUYÊN tên người, địa danh, tổ chức, vật phẩm riêng.\n\
- GIỮ NGUYÊN mọi thẻ như %s, {..}, [..], <..>, \\n, \\c[1] và vị trí của chúng.\n\
- Chữ đọc từ màn hình có thể sai chính tả do OCR: tự đoán và sửa khi dịch.\n\
- Chỉ trả về bản dịch, không giải thích.";

/// Bật khi mã bị từ chối (401/403) → chuyển sang Google.
pub static KHOA_HONG: AtomicBool = AtomicBool::new(false);

pub fn co_khoa() -> bool {
    !KHOA_HONG.load(Ordering::Relaxed) && cau_hinh::doc_khoa("khoa_api.txt").is_some()
}

fn goi(he_thong: &str, noi_dung: &str, toi_da: u32) -> Result<String> {
    let khoa = cau_hinh::doc_khoa("khoa_api.txt").ok_or_else(|| anyhow!("Chưa có mã API Claude"))?;
    if KHOA_HONG.load(Ordering::Relaxed) {
        bail!("Mã API Claude bị từ chối");
    }
    let than = serde_json::json!({
        "model": MO_HINH,
        "max_tokens": toi_da,
        "system": he_thong,
        "messages": [{"role": "user", "content": noi_dung}],
    });
    let tra = ureq::post("https://api.anthropic.com/v1/messages")
        .set("x-api-key", &khoa)
        .set("anthropic-version", "2023-06-01")
        .timeout(Duration::from_secs(120))
        .send_json(than);
    let tra: serde_json::Value = match tra {
        Ok(r) => r.into_json()?,
        Err(ureq::Error::Status(ma @ (401 | 403), _)) => {
            KHOA_HONG.store(true, Ordering::Relaxed);
            bail!("Mã API Claude bị từ chối ({ma}) – chuyển sang Google");
        }
        Err(ureq::Error::Status(ma, r)) => bail!("Claude lỗi {ma}: {}", r.into_string().unwrap_or_default()),
        Err(e) => return Err(e.into()),
    };
    let mut ra = String::new();
    for k in tra["content"].as_array().into_iter().flatten() {
        if k["type"] == "text" {
            ra.push_str(k["text"].as_str().unwrap_or(""));
        }
    }
    Ok(ra.trim().to_string())
}

pub fn dich(chu: &str) -> Result<String> {
    goi(HUONG_DAN, chu, 1024)
}

/// Dịch theo lô: gửi mảng JSON, nhận mảng JSON cùng số phần tử.
pub fn dich_lo(cac_cau: &[String], he_thong: &str) -> Result<Vec<String>> {
    let yeu_cau = format!(
        "Dịch từng phần tử của mảng JSON sau sang tiếng Việt. Trả về DUY NHẤT một mảng JSON có đúng {} phần tử, cùng thứ tự.\n{}",
        cac_cau.len(),
        serde_json::to_string(cac_cau)?
    );
    let tra = goi(he_thong, &yeu_cau, 8000)?;
    let dau = tra.find('[').ok_or_else(|| anyhow!("Claude không trả mảng JSON"))?;
    let cuoi = tra.rfind(']').ok_or_else(|| anyhow!("Claude không trả mảng JSON"))?;
    let ra: Vec<String> = serde_json::from_str(&tra[dau..=cuoi])?;
    if ra.len() != cac_cau.len() {
        bail!("Claude trả {} câu, cần {}", ra.len(), cac_cau.len());
    }
    Ok(ra)
}
