//! Google Translate (gtx) – miễn phí, cần mạng. Dùng dự phòng / hiện ngay.

use anyhow::{anyhow, Result};
use std::time::Duration;

pub fn dich(chu: &str) -> Result<String> {
    let tra: serde_json::Value = ureq::get("https://translate.googleapis.com/translate_a/single")
        .query("client", "gtx")
        .query("sl", "auto")
        .query("tl", "vi")
        .query("dt", "t")
        .query("q", chu)
        .timeout(Duration::from_secs(8))
        .call()?
        .into_json()?;
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
