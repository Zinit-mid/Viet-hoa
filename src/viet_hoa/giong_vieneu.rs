//! Lõi giọng VieNeu dùng chung cho mọi game (GĐ6): chạy `scripts\tao_giong.py` bằng Python của
//! app VieNeu (bản GPU ưu tiên), đọc các câu thành WAV 48 kHz theo giọng mẫu từng nhóm người nói.
//! Chuyển từ `ViecDich.tim_vieneu / chay_vieneu` + KICH_BAN_VIENEU của app cũ.
//! Có file `_da_tao.txt` mỗi nhóm → dừng rồi chạy tiếp không đọc lại câu đã xong.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Result};
use serde_json::json;

use crate::duong_dan;

/// Một người nói: giọng mẫu + các câu cần đọc.
pub struct Nhom {
    pub ten: String,
    /// file wav giọng mẫu (6–15 s rõ tiếng)
    pub mau: PathBuf,
    /// thư mục ra: `<id>.wav`
    pub ra: PathBuf,
    /// độ to (RMS) mong muốn, mặc định 0.08
    pub rms: f64,
    /// (id file, câu tiếng Việt, thời lượng giọng gốc giây; 0 = không ép thời gian)
    pub ds: Vec<(String, String, f64)>,
}

pub struct Vieneu {
    pub python: PathBuf,
    pub hf_home: PathBuf,
    pub gpu: bool,
}

/// Python của app VieNeu: `%LOCALAPPDATA%\com.vieneu.app\{gpu|dubtools}\venv`.
pub fn tim_vieneu() -> Option<Vieneu> {
    let goc = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("com.vieneu.app");
    for (loai, gpu) in [("gpu", true), ("dubtools", false)] {
        let venv = goc.join(loai).join("venv");
        let py = venv.join("Scripts").join("python.exe");
        if py.is_file() && venv.join("Lib").join("site-packages").join("vieneu").is_dir() {
            return Some(Vieneu { python: py, hf_home: goc.join(loai).join("hf"), gpu });
        }
    }
    None
}

/// Ghi viec.json cho script (tách riêng để kiểm thử).
pub fn viec_json(nhom: &[Nhom], gpu: bool, toi_da: f64) -> serde_json::Value {
    json!({
        "gpu": gpu,
        "toi_da": toi_da,
        "nhom": nhom.iter().map(|g| json!({
            "ten": g.ten,
            "mau": g.mau.to_string_lossy(),
            "ra": g.ra.to_string_lossy(),
            "rms": g.rms,
            "ds": g.ds.iter().map(|(i, c, t)| json!([i, c, t])).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// Chạy VieNeu. `tien_do(xong, tong, ghi_chú)`. → Ok(true) xong, Ok(false) bị dừng.
/// `tam` = thư mục để ghi viec.json và nhật ký.
pub fn chay(
    nhom: &[Nhom],
    tam: &Path,
    toi_da: f64,
    dung: &AtomicBool,
    tien_do: &dyn Fn(usize, usize, &str),
) -> Result<bool> {
    let vn = tim_vieneu().ok_or_else(|| {
        anyhow!("không tìm thấy app VieNeu (bản tăng tốc GPU) – mở VieNeu, cài 'GPU' rồi thử lại")
    })?;
    std::fs::create_dir_all(tam)?;
    let file_viec = tam.join("viec.json");
    std::fs::write(&file_viec, serde_json::to_vec(&viec_json(nhom, vn.gpu, toi_da))?)?;
    let kich_ban = duong_dan::goc().join("scripts").join("tao_giong.py");
    if !kich_ban.is_file() {
        return Err(anyhow!("thiếu {}", kich_ban.display()));
    }
    let file_nk = tam.join("vieneu_nhat_ky.txt");
    let mut nk = std::fs::OpenOptions::new().create(true).append(true).open(&file_nk)?;

    let mut lenh = Command::new(&vn.python);
    lenh.arg("-I")
        .arg(&kich_ban)
        .arg(&file_viec)
        .env("HF_HOME", &vn.hf_home)
        .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .current_dir(tam)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        lenh.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    // stderr gộp vào nhật ký qua file riêng
    lenh.stderr(Stdio::from(std::fs::OpenOptions::new().create(true).append(true).open(tam.join("vieneu_loi.txt"))?));

    tien_do(0, 0, if vn.gpu { "đang nạp mô hình (card màn hình)..." } else { "đang nạp mô hình (CPU – sẽ rất chậm)..." });
    let mut con = lenh.spawn()?;
    let out = con.stdout.take().ok_or_else(|| anyhow!("không đọc được đầu ra VieNeu"))?;
    let (mut bat_dau, mut nhom_hien) = (std::time::Instant::now(), String::new());
    let mut loi_cuoi: Vec<String> = Vec::new();
    for dong in BufReader::new(out).lines() {
        let d = dong.unwrap_or_default();
        let _ = writeln!(nk, "{d}");
        if dung.load(Ordering::Relaxed) {
            let _ = con.kill();
            let _ = con.wait();
            return Ok(false);
        }
        let t: Vec<&str> = d.split_whitespace().collect();
        match t.first().copied() {
            Some("XONG") if t.len() >= 3 => {
                let (a, b): (usize, usize) = (t[1].parse().unwrap_or(0), t[2].parse().unwrap_or(0));
                let con_lai = bat_dau.elapsed().as_secs_f64() / a.max(1) as f64 * b.saturating_sub(a) as f64;
                tien_do(a, b, &format!("{nhom_hien} • còn khoảng {:.0} phút", con_lai / 60.0));
            }
            Some("NHOM") if t.len() >= 4 => nhom_hien = format!(" • giọng {}/{}: {}", t[1], t[2], t[3]),
            Some("TONG") => bat_dau = std::time::Instant::now(),
            Some("HET") | None => {}
            _ => loi_cuoi = {
                loi_cuoi.push(d.clone());
                loi_cuoi.split_off(loi_cuoi.len().saturating_sub(6))
            },
        }
    }
    let st = con.wait()?;
    if dung.load(Ordering::Relaxed) {
        return Ok(false);
    }
    if !st.success() {
        let cuoi: Vec<_> = loi_cuoi.iter().rev().take(3).rev().cloned().collect();
        return Err(anyhow!("VieNeu dừng giữa chừng – xem {}\n{}", file_nk.display(), cuoi.join("\n")));
    }
    Ok(true)
}

/// Chạy một script phụ trong `scripts\` bằng Python của VieNeu (phân tích / giọng mẫu / ghép MP3...).
/// Script đọc argv[1] = file JSON và in `XONG a b` để báo tiến độ. → Ok(false) nếu bị dừng.
pub fn chay_kich_ban(
    ten_script: &str,
    viec: &serde_json::Value,
    tam: &Path,
    nhan: &str,
    dung: &AtomicBool,
    bao: &dyn Fn(&str),
) -> Result<bool> {
    let vn = tim_vieneu().ok_or_else(|| anyhow!("không tìm thấy app VieNeu (bản tăng tốc GPU)"))?;
    std::fs::create_dir_all(tam)?;
    let tep = tam.join(format!("{ten_script}.json"));
    std::fs::write(&tep, serde_json::to_vec(viec)?)?;
    let kb = duong_dan::goc().join("scripts").join(ten_script);
    if !kb.is_file() {
        return Err(anyhow!("thiếu {}", kb.display()));
    }
    let mut lenh = Command::new(&vn.python);
    lenh.arg("-I")
        .arg(&kb)
        .arg(&tep)
        .env("HF_HOME", &vn.hf_home)
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .current_dir(tam)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        lenh.creation_flags(0x0800_0000);
    }
    let mut con = lenh.spawn()?;
    // stderr đọc ở luồng riêng để không nghẽn ống
    let err = con.stderr.take().map(|mut e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            let _ = std::io::Read::read_to_string(&mut e, &mut s);
            s
        })
    });
    let out = con.stdout.take().ok_or_else(|| anyhow!("không đọc được đầu ra"))?;
    let mut cuoi: Vec<String> = Vec::new();
    for dong in BufReader::new(out).lines() {
        let d = dong.unwrap_or_default();
        if dung.load(Ordering::Relaxed) {
            let _ = con.kill();
            let _ = con.wait();
            return Ok(false);
        }
        let t: Vec<&str> = d.split_whitespace().collect();
        if t.first() == Some(&"XONG") && t.len() >= 3 {
            bao(&format!("{nhan} {}/{}", t[1], t[2]));
        } else if !d.trim().is_empty() && !d.starts_with("HET") && !d.starts_with("TONG") {
            cuoi.push(d);
            if cuoi.len() > 4 {
                cuoi.remove(0);
            }
        }
    }
    let st = con.wait()?;
    let loi = err.and_then(|h| h.join().ok()).unwrap_or_default();
    if dung.load(Ordering::Relaxed) {
        return Ok(false);
    }
    if !st.success() {
        let l: Vec<&str> = loi.lines().rev().take(3).collect();
        return Err(anyhow!("{nhan} bị lỗi:
{}
{}", cuoi.join("
"), l.into_iter().rev().collect::<Vec<_>>().join("
")));
    }
    Ok(true)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn viec_json_dung_dang() {
        let g = Nhom {
            ten: "kliff".into(),
            mau: "m.wav".into(),
            ra: "ra".into(),
            rms: 0.08,
            ds: vec![("a1".into(), "Xin chào".into(), 1.5)],
        };
        let j = viec_json(&[g], true, 1.4);
        assert_eq!(j["nhom"][0]["ds"][0][1], "Xin chào");
        assert_eq!(j["toi_da"], 1.4);
        assert_eq!(j["gpu"], true);
    }

    /// Đọc thật 2 câu: `MAU=<wav> cargo test --release -- --ignored vieneu_that`
    #[test]
    #[ignore]
    fn vieneu_that() {
        let mau = std::env::var("MAU").expect("MAU=<file wav giọng mẫu>");
        let tam = std::env::temp_dir().join("vh_vieneu_test");
        let _ = std::fs::remove_dir_all(&tam);
        let g = Nhom {
            ten: "thu".into(),
            mau: mau.into(),
            ra: tam.join("ra"),
            rms: 0.08,
            ds: vec![
                ("c1".into(), "Xin chào, tôi là người lính.".into(), 0.0),
                ("c2".into(), "Hãy cẩn thận, phía trước có kẻ thù.".into(), 2.0),
            ],
        };
        let dung = AtomicBool::new(false);
        let xong = chay(&[g], &tam, 1.4, &dung, &|a, b, s| eprintln!("{a}/{b} {s}")).unwrap();
        assert!(xong);
        for f in ["c1.wav", "c2.wav"] {
            assert!(std::fs::metadata(tam.join("ra").join(f)).unwrap().len() > 10_000);
        }
    }
}
