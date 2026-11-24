//! Clone giọng: file ghi âm → giọng mẫu sạch (cắt theo giây, mono 24 kHz, lọc 70 Hz, bỏ lặng,
//! chuẩn hóa độ to) → nghe thử bằng VieNeu → lưu vào kho giọng (`<ổ>:\DichGame_LongTieng\giong_rieng`,
//! dùng chung với app cũ) → gán cho người nói từng game (`phan_bo.json`: "A", "B" = Crimson Desert,
//! "TR2013" = Tomb Raider 2013).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

use anyhow::{anyhow, bail, Result};

use super::{giong_vieneu as gv, wwise};
use crate::duong_dan;

pub const TAN_SO: u32 = 24000;
const RMS_DICH: f64 = 0.08;

/// Thư mục kho giọng: thư mục `giong_rieng` đã có trên các ổ, nếu chưa có thì tạo cạnh dữ liệu app.
pub fn kho_giong() -> PathBuf {
    for o in "GFEDCHIJK".chars() {
        let p = PathBuf::from(format!(r"{o}:\DichGame_LongTieng\giong_rieng"));
        if p.is_dir() {
            return p;
        }
    }
    let g = duong_dan::du_lieu().join("giong_rieng");
    let _ = std::fs::create_dir_all(&g);
    g
}

/// Các file .wav trong kho (không tính bản cũ *_cu.wav), A–Z.
pub fn liet_ke() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(kho_giong())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            let l = n.to_lowercase();
            l.ends_with(".wav") && !l.ends_with("_cu.wav")
        })
        .collect();
    v.sort_by_key(|n| n.to_lowercase());
    v
}

/// Đọc file ghi âm: WAV đọc thẳng, định dạng khác (mp3, m4a, mp4, ogg...) qua ffmpeg.
pub fn doc_nguon(p: &Path) -> Result<(Vec<f32>, u32)> {
    if let Ok(b) = std::fs::read(p) {
        if let Ok(r) = wwise::doc_wav(&b) {
            return Ok(r);
        }
    }
    let ff = duong_dan::cong_cu().join("ffmpeg").join("bin").join("ffmpeg.exe");
    if !ff.is_file() {
        bail!("không đọc được file (không phải WAV) và thiếu {}", ff.display());
    }
    let tam = duong_dan::du_lieu().join("tam");
    std::fs::create_dir_all(&tam)?;
    let ra = tam.join("clone_nguon.wav");
    let _ = std::fs::remove_file(&ra);
    let mut l = Command::new(ff);
    l.args(["-y", "-v", "error", "-i"])
        .arg(p)
        .args(["-vn", "-ac", "1", "-ar", "48000", "-c:a", "pcm_s16le"])
        .arg(&ra)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        l.creation_flags(0x0800_0000);
    }
    if !l.status().is_ok_and(|s| s.success()) || !ra.is_file() {
        bail!("ffmpeg không đọc được file này");
    }
    wwise::doc_wav(&std::fs::read(&ra)?)
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

/// Đổi tần số: lọc trung bình (chống răng cưa khi hạ) rồi nội suy tuyến tính.
fn doi_tan_so(x: &[f32], tu: u32, den: u32) -> Vec<f32> {
    if tu == den || x.len() < 2 {
        return x.to_vec();
    }
    let mut y: Vec<f32> = x.to_vec();
    if tu > den {
        let w = (tu as f64 / den as f64).round().max(1.0) as usize;
        if w > 1 {
            let mut s = 0f64;
            y = (0..x.len())
                .map(|i| {
                    s += x[i] as f64;
                    if i >= w {
                        s -= x[i - w] as f64;
                    }
                    (s / w.min(i + 1) as f64) as f32
                })
                .collect();
        }
    }
    let n = (y.len() as f64 * den as f64 / tu as f64) as usize;
    (0..n)
        .map(|i| {
            let p = i as f64 * tu as f64 / den as f64;
            let a = (p as usize).min(y.len() - 1);
            let f = (p - a as f64) as f32;
            y[a] + (y[(a + 1).min(y.len() - 1)] - y[a]) * f
        })
        .collect()
}

/// Lọc thông cao 2 cực (Butterworth) – bỏ tiếng ù / gió dưới `fc` Hz.
fn loc_cao(x: &mut [f32], sr: u32, fc: f64) {
    let w = 2.0 * std::f64::consts::PI * fc / sr as f64;
    let (cs, sn) = (w.cos(), w.sin());
    let al = sn / std::f64::consts::SQRT_2;
    let a0 = 1.0 + al;
    let b0 = (1.0 + cs) / 2.0 / a0;
    let b1 = -(1.0 + cs) / a0;
    let b2 = b0;
    let a1 = -2.0 * cs / a0;
    let a2 = (1.0 - al) / a0;
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    for v in x.iter_mut() {
        let x0 = *v as f64;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        (x2, x1, y2, y1) = (x1, x0, y1, y0);
        *v = y0 as f32;
    }
}

/// Bỏ lặng đầu / cuối, rút các khoảng lặng giữa quá dài còn 0,3 giây (khung 20 ms).
fn bo_lang(x: &[f32], sr: u32) -> Vec<f32> {
    let kh = (sr as usize / 50).max(1);
    let e: Vec<f64> = x.chunks(kh).map(rms).collect();
    let dinh = e.iter().cloned().fold(0.0, f64::max);
    if dinh < 0.003 {
        return Vec::new();
    }
    let nguong = (dinh * 0.06).max(0.004);
    let co: Vec<bool> = e.iter().map(|v| *v > nguong).collect();
    let (Some(dau), Some(cuoi)) = (co.iter().position(|&c| c), co.iter().rposition(|&c| c)) else {
        return Vec::new();
    };
    let toi_da = 25usize; // 0,5 s
    let giu = 15usize; // 0,3 s
    let mut ra: Vec<f32> = Vec::new();
    let mut i = dau;
    while i <= cuoi {
        if co[i] {
            ra.extend_from_slice(&x[i * kh..((i + 1) * kh).min(x.len())]);
            i += 1;
        } else {
            let mut j = i;
            while j <= cuoi && !co[j] {
                j += 1;
            }
            let dai = (j - i).min(if j - i > toi_da { giu } else { j - i });
            ra.extend_from_slice(&x[i * kh..((i + dai) * kh).min(x.len())]);
            i = j;
        }
    }
    ra
}

/// Giọng mẫu sạch từ đoạn [bat_dau, ket_thuc] giây (ket_thuc <= 0 = hết file). Mono 24 kHz, tối đa 20 giây.
pub fn chuan_bi(x: &[f32], sr: u32, bat_dau: f64, ket_thuc: f64) -> Result<Vec<f32>> {
    let a = ((bat_dau.max(0.0) * sr as f64) as usize).min(x.len());
    let b = if ket_thuc > bat_dau { ((ket_thuc * sr as f64) as usize).min(x.len()) } else { x.len() };
    if b <= a {
        bail!("đoạn cắt trống – kiểm tra giây bắt đầu / kết thúc");
    }
    let mut y = doi_tan_so(&x[a..b], sr, TAN_SO);
    loc_cao(&mut y, TAN_SO, 70.0);
    let mut y = bo_lang(&y, TAN_SO);
    if (y.len() as f64) < 1.5 * TAN_SO as f64 {
        bail!("đoạn có tiếng nói quá ngắn (cần ít nhất ~3 giây, tốt nhất 6–15 giây rõ tiếng, không nhạc nền)");
    }
    y.truncate(20 * TAN_SO as usize);
    let r = rms(&y);
    if r > 1e-5 {
        let g = (RMS_DICH / r) as f32;
        for v in y.iter_mut() {
            let s = *v * g;
            *v = if s.abs() <= 0.85 { s } else { s.signum() * (0.85 + 0.15 * ((s.abs() - 0.85) / 0.15).tanh()) };
        }
    }
    Ok(y)
}

pub fn thong_ke(y: &[f32]) -> (f64, f64) {
    (y.len() as f64 / TAN_SO as f64, rms(y))
}

/// Đọc câu `chu` bằng giọng mẫu này (VieNeu). → file WAV kết quả.
pub fn nghe_thu(
    mau: &[f32],
    chu: &str,
    dung: &AtomicBool,
    bao: &dyn Fn(&str),
) -> Result<Option<PathBuf>> {
    let tam = duong_dan::du_lieu().join("tam").join("clone_thu");
    let _ = std::fs::remove_dir_all(&tam);
    std::fs::create_dir_all(&tam)?;
    let mau_wav = tam.join("mau.wav");
    wwise::ghi_wav16(&mau_wav, mau, TAN_SO)?;
    let chu = super::phien_am::doc_duoc(chu.trim());
    if chu.is_empty() {
        bail!("chưa nhập câu để đọc thử");
    }
    let nhom = gv::Nhom {
        ten: "thu".into(),
        mau: mau_wav,
        ra: tam.join("ra"),
        rms: RMS_DICH,
        ds: vec![("thu".into(), chu, 0.0)],
    };
    let xong = gv::chay(&[nhom], &tam, 1.4, dung, &|_, _, s| bao(&format!("VieNeu đang đọc: {}", s.trim_start_matches(" • "))))?;
    let ra = tam.join("ra").join("thu.wav");
    if !xong {
        return Ok(None);
    }
    if ra.is_file() {
        Ok(Some(ra))
    } else {
        Err(anyhow!("VieNeu không tạo được giọng – xem {}", tam.join("vieneu_nhat_ky.txt").display()))
    }
}

/// Lưu giọng mẫu vào kho (không ghi đè file đã có, trừ khi `ghi_de`). → đường dẫn file.
pub fn luu(ten: &str, mau: &[f32], ghi_de: bool) -> Result<PathBuf> {
    let ten: String = ten
        .trim()
        .trim_end_matches(".wav")
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if ten.is_empty() {
        bail!("chưa đặt tên giọng");
    }
    let p = kho_giong().join(format!("{ten}.wav"));
    if p.exists() && !ghi_de {
        bail!("đã có giọng tên {ten}.wav trong kho – đặt tên khác hoặc bấm ghi đè");
    }
    wwise::ghi_wav16(&p, mau, TAN_SO)?;
    Ok(p)
}

fn tep_phan_bo() -> PathBuf {
    kho_giong().join("phan_bo.json")
}

pub fn doc_phan_bo() -> serde_json::Value {
    std::fs::read(tep_phan_bo())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Gán giọng `file` cho `nguoi` ở phần `phan` ("A", "B", "TR2013"). Sao lưu phan_bo.json 1 lần,
/// ghi nhớ người đã gán tay (`_tay`) để app không tự đổi giới tính của họ.
pub fn gan(phan: &str, nguoi: &str, file: &str) -> Result<()> {
    let nguoi = nguoi.trim();
    if nguoi.is_empty() || file.is_empty() {
        bail!("chưa chọn người nói hoặc giọng");
    }
    let tep = tep_phan_bo();
    let sao = PathBuf::from(format!("{}.truoc_gan_tay", tep.display()));
    if tep.is_file() && !sao.exists() {
        std::fs::copy(&tep, &sao)?;
    }
    let mut pb = doc_phan_bo();
    let goc = pb.as_object_mut().ok_or_else(|| anyhow!("phan_bo.json hỏng"))?;
    let ds = goc.entry(phan).or_insert_with(|| serde_json::json!({}));
    ds.as_object_mut().ok_or_else(|| anyhow!("phan_bo.json hỏng"))?.insert(nguoi.into(), file.into());
    if phan != "TR2013" {
        let tay = goc.entry("_tay").or_insert_with(|| serde_json::json!({}));
        let l = tay.as_object_mut().ok_or_else(|| anyhow!("phan_bo.json hỏng"))?
            .entry(phan).or_insert_with(|| serde_json::json!([]));
        if let Some(a) = l.as_array_mut() {
            if !a.iter().any(|v| v.as_str() == Some(nguoi)) {
                a.push(nguoi.into());
                a.sort_by_key(|v| v.as_str().unwrap_or("").to_string());
            }
        }
    }
    let t = PathBuf::from(format!("{}.tam", tep.display()));
    std::fs::write(&t, serde_json::to_vec_pretty(&pb)?)?;
    std::fs::rename(&t, &tep)?;
    Ok(())
}

/// Người nói đã có trong phan_bo.json của phần `phan` (gợi ý khi gán).
pub fn nguoi_da_co(phan: &str) -> Vec<String> {
    doc_phan_bo()
        .get(phan)
        .and_then(|v| v.as_object())
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod test {
    use super::*;

    fn tieng(ts: u32, giay: f64, bien: f32) -> Vec<f32> {
        (0..(ts as f64 * giay) as usize)
            .map(|i| bien * (i as f32 * 2.0 * std::f32::consts::PI * 200.0 / ts as f32).sin() * (1.0 + 0.5 * (i as f32 * 0.001).sin()))
            .collect()
    }

    #[test]
    fn chuan_bi_cat_va_bo_lang() {
        let mut x = vec![0f32; 44100]; // 1 s lặng
        x.extend(tieng(44100, 5.0, 0.3));
        x.extend(vec![0f32; 44100 * 3]); // 3 s lặng
        x.extend(tieng(44100, 2.0, 0.3));
        let y = chuan_bi(&x, 44100, 0.0, 0.0).unwrap();
        let (giay, r) = thong_ke(&y);
        // 5 + 0,3 (lặng rút gọn) + 2 ≈ 7,3 s
        assert!((7.0..7.8).contains(&giay), "{giay}");
        assert!((r - RMS_DICH).abs() < 0.02, "{r}");
        // cắt giây 1–6 chỉ còn đoạn tiếng đầu
        let z = chuan_bi(&x, 44100, 1.0, 6.0).unwrap();
        assert!((4.5..5.2).contains(&thong_ke(&z).0));
        assert!(chuan_bi(&x, 44100, 7.0, 7.5).is_err());
    }

    #[test]
    fn loc_cao_bo_tieng_u() {
        let mut u = tieng(24000, 1.0, 0.5);
        let mut thap: Vec<f32> = (0..24000).map(|i| 0.5 * (i as f32 * 2.0 * std::f32::consts::PI * 20.0 / 24000.0).sin()).collect();
        loc_cao(&mut thap, 24000, 70.0);
        loc_cao(&mut u, 24000, 70.0);
        assert!(rms(&thap[4000..]) < 0.05, "{}", rms(&thap[4000..]));
        assert!(rms(&u[4000..]) > 0.2);
    }

    /// Thật: FILE=<wav/mp3> cargo test --release -- --ignored clone_that --nocapture
    #[test]
    #[ignore]
    fn clone_that() {
        let (x, ts) = doc_nguon(Path::new(&std::env::var("FILE").unwrap())).unwrap();
        let y = chuan_bi(&x, ts, 0.0, 0.0).unwrap();
        println!("mẫu {:?}", thong_ke(&y));
        let dung = AtomicBool::new(false);
        let ra = nghe_thu(&y, "Xin chào, tôi là Kliff. Hãy cẩn thận phía trước.", &dung, &|s| println!("{s}"))
            .unwrap()
            .unwrap();
        let (z, zs) = wwise::doc_wav(&std::fs::read(ra).unwrap()).unwrap();
        println!("đọc thử {:.1}s {zs}Hz", z.len() as f64 / zs as f64);
        assert!(z.len() as u32 > zs);
    }
}
