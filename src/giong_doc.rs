//! Đọc bản dịch thành tiếng: edge-tts (Python riêng) → mp3 → phát bằng MCI.
//! Câu mới ngắt câu cũ.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use windows::core::HSTRING;
use windows::Win32::Media::Multimedia::mciSendStringW;

use crate::duong_dan;

static SO: AtomicU64 = AtomicU64::new(0);
static GIONG: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Đặt giọng đọc (vi-VN-NamMinhNeural / vi-VN-HoaiMyNeural).
pub fn dat_giong(g: &str) {
    if let Ok(mut k) = GIONG.lock() {
        if *k != g {
            *k = g.to_string();
        }
    }
}

fn mci(lenh: &str) {
    unsafe {
        let _ = mciSendStringW(&HSTRING::from(lenh), None, None);
    }
}

pub fn doc(chu: String) {
    let giong = GIONG.lock().map(|g| g.clone()).unwrap_or_default();
    let giong = if giong.is_empty() { "vi-VN-NamMinhNeural".to_string() } else { giong };
    let Some(py) = duong_dan::python() else { return };
    let so = SO.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let thu_muc = duong_dan::du_lieu().join("tam");
        let _ = std::fs::create_dir_all(&thu_muc);
        let mp3 = thu_muc.join(format!("giong_{}.mp3", so % 4));
        // Bỏ tên người nói "Tên: ..." khi đọc
        let chu = match chu.split_once(": ") {
            Some((ten, sau)) if ten.chars().count() <= 20 => sau.to_string(),
            _ => chu,
        };
        let ok = Command::new(py)
            .args(["-I", "-m", "edge_tts", "--voice", &giong, "--text", &chu, "--write-media"])
            .arg(&mp3)
            .creation_flags(0x0800_0000)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok || SO.load(Ordering::SeqCst) != so {
            return; // đã có câu mới hơn
        }
        mci("close vh_giong");
        mci(&format!("open \"{}\" type mpegvideo alias vh_giong", mp3.display()));
        mci("play vh_giong");
    });
}

pub fn dung() {
    SO.fetch_add(1, Ordering::SeqCst);
    mci("close vh_giong");
}
