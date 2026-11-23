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
/// Người dùng bật / tắt Claude (Ctrl+Shift+C, cấu hình "claude_bat").
pub static BAT: AtomicBool = AtomicBool::new(true);

pub fn co_khoa() -> bool {
    BAT.load(Ordering::Relaxed) && !KHOA_HONG.load(Ordering::Relaxed) && cau_hinh::doc_khoa("khoa_api.txt").is_some()
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

/// Lỗi không thể tiếp tục (sai mã, hết tiền) – dừng cả lượt dịch (giống LoiDung của app cũ).
#[derive(Debug)]
pub struct LoiDung(pub String);

impl std::fmt::Display for LoiDung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for LoiDung {}

/// Gọi Claude cho việc dịch dài (Việt hóa): tự thử lại khi bận / mất mạng (tối đa 6 lần),
/// trả (chữ, token vào, token ra). Sai mã / hết tiền → LoiDung.
pub fn goi_day_du(he_thong: &str, noi_dung: &str, toi_da: u32) -> Result<(String, u64, u64)> {
    let khoa = cau_hinh::doc_khoa("khoa_api.txt")
        .ok_or_else(|| LoiDung("chưa có mã API Claude (Cài đặt > Mã API)".into()))?;
    let than = serde_json::json!({
        "model": MO_HINH,
        "max_tokens": toi_da,
        "system": he_thong,
        "messages": [{"role": "user", "content": noi_dung}],
    });
    for lan in 0..6u64 {
        let tra = ureq::post("https://api.anthropic.com/v1/messages")
            .set("x-api-key", &khoa)
            .set("anthropic-version", "2023-06-01")
            .timeout(Duration::from_secs(180))
            .send_json(than.clone());
        let tra: serde_json::Value = match tra {
            Ok(r) => r.into_json()?,
            Err(ureq::Error::Status(401 | 403, _)) => {
                KHOA_HONG.store(true, Ordering::Relaxed);
                return Err(LoiDung("mã Claude không hợp lệ (HTTP 401/403) – kiểm tra lại mã trong Cài đặt".into()).into());
            }
            Err(ureq::Error::Status(429 | 500 | 502 | 503 | 529, _)) => {
                std::thread::sleep(Duration::from_secs(5 + lan * 10));
                continue;
            }
            Err(ureq::Error::Status(ma, r)) => {
                let tb = r.into_string().unwrap_or_default();
                let tb: String = serde_json::from_str::<serde_json::Value>(&tb)
                    .ok()
                    .and_then(|v| v["error"]["message"].as_str().map(String::from))
                    .unwrap_or_else(|| tb.chars().take(200).collect());
                let thap = tb.to_lowercase();
                if thap.contains("credit") || thap.contains("billing") {
                    return Err(LoiDung(format!("tài khoản Claude hết tiền: {tb}")).into());
                }
                bail!("HTTP {ma}: {tb}");
            }
            Err(_) => {
                std::thread::sleep(Duration::from_secs(2 + lan * 3));
                continue;
            }
        };
        let mut chu = String::new();
        for k in tra["content"].as_array().into_iter().flatten() {
            if k["type"] == "text" {
                chu.push_str(k["text"].as_str().unwrap_or(""));
            }
        }
        let vao = tra["usage"]["input_tokens"].as_u64().unwrap_or(0);
        let ra = tra["usage"]["output_tokens"].as_u64().unwrap_or(0);
        return Ok((chu, vao, ra));
    }
    bail!("Claude bận hoặc mất mạng, đã thử lại nhiều lần")
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
