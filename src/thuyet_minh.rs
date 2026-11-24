//! Thuyết minh phim tiếng Việt: đọc từng dòng phụ đề đã dịch bằng một giọng chuẩn (VieNeu), có cảm xúc,
//! đặt đúng thời điểm, hạ tiếng gốc khi thuyết minh nói, trộn ra file phim mới (hoặc file âm thanh).
//! Cảm xúc làm bằng hai cách kết hợp:
//!   1. Nhấn nhá theo câu: đoán loại câu từ dấu câu / từ khóa → chỉnh to nhỏ, nhanh chậm, cao thấp.
//!   2. Giọng mẫu theo cảm xúc: mỗi giọng có bộ 3 mẫu (bình thường / gấp rút / buồn); câu nào dùng mẫu đó.

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, bail, Result};
use serde_json::json;

use crate::duong_dan;
use crate::viet_hoa::cd_giong::lam_sach_loi;
use crate::viet_hoa::{giong_vieneu as gv, phien_am, wwise};

/// 6 giọng thuyết minh (kể chuyện / đọc truyện) trong số 25 giọng dựng sẵn của VieNeu.
pub const GIONG: [(&str, &str); 6] = [
    ("Thiện Minh", "Nam • kể chuyện"),
    ("Thái Sơn", "Nam • kể chuyện"),
    ("Đức Trí", "Nam • đọc truyện"),
    ("Ngọc Linh", "Nữ • kể chuyện"),
    ("Mỹ Duyên", "Nữ • đọc truyện"),
    ("Thục Đoan", "Nữ • kể chuyện"),
];

#[derive(Clone)]
pub struct Tuy {
    pub giong: String,
    /// cách 2: giọng mẫu theo cảm xúc
    pub theo_cam_xuc: bool,
    /// cách 1: nhấn nhá theo câu
    pub nhan_nha: bool,
    /// độ mạnh nhấn nhá 0.3–1.0
    pub cuong_do: f64,
    /// chỉ làm N dòng đầu để nghe thử
    pub thu_dau: Option<usize>,
}

/// Đoán cảm xúc từ câu tiếng Việt (ưu tiên: gấp rút > nhấn mạnh > buồn > hỏi > bình thường).
pub fn phan_loai(vi: &str) -> &'static str {
    let t = vi.to_lowercase();
    let co = |ds: &[&str]| ds.iter().any(|k| t.contains(k));
    let cham_than = t.matches('!').count();
    if (cham_than >= 1 && co(&["chạy", "mau", "nhanh lên", "cẩn thận", "nguy hiểm", "giết", "tránh", "cứu", "khẩn", "tấn công", "bắn", "nổ", "đừng"])) || cham_than >= 2 {
        "gap_rut"
    } else if cham_than == 1 {
        "nhan_manh"
    } else if t.contains("...") || t.contains('…') || co(&["khóc", "nước mắt", "buồn", "đã chết", "cô đơn", "xin lỗi", "tạm biệt", "vĩnh biệt", "đau đớn", "tiếc", "mất rồi"]) {
        "buon"
    } else if t.trim_end().ends_with('?') {
        "hoi"
    } else {
        "binh_thuong"
    }
}

pub fn thu_muc_bo() -> PathBuf {
    duong_dan::du_lieu().join("giong_thuyet_minh")
}

fn thu_muc_giong(g: &str) -> PathBuf {
    thu_muc_bo().join(g.replace(' ', "_"))
}

/// Bộ giọng cảm xúc của `g` đã đủ 3 mẫu chưa.
pub fn co_bo(g: &str) -> bool {
    ["binh_thuong", "gap_rut", "buon"].iter().all(|k| thu_muc_giong(g).join(format!("{k}.wav")).is_file())
}

/// Tạo bộ giọng cảm xúc (3 mẫu) cho các giọng dựng sẵn. → false nếu bị dừng.
pub fn tao_bo(giongs: &[&str], dung: &AtomicBool, bao: &dyn Fn(&str)) -> Result<bool> {
    let tam = duong_dan::du_lieu().join("tam").join("thuyet_minh");
    gv::chay_kich_ban(
        "thuyet_minh.py",
        &json!({"che_do": "tao_bo", "giong": giongs, "ra": thu_muc_bo().to_string_lossy()}),
        &tam,
        "Tạo bộ giọng cảm xúc",
        dung,
        bao,
    )
}

fn ffbin(t: &str) -> PathBuf {
    duong_dan::cong_cu().join("ffmpeg").join("bin").join(t)
}

fn an(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

/// (giây, có hình không)
fn thong_tin(p: &Path) -> Result<(f64, bool)> {
    let mut c = Command::new(ffbin("ffprobe.exe"));
    an(&mut c)
        .args(["-v", "error", "-show_entries", "format=duration:stream=codec_type", "-of", "json"])
        .arg(p)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let o = c.output()?;
    let v: serde_json::Value = serde_json::from_slice(&o.stdout)?;
    let giay = v["format"]["duration"].as_str().and_then(|x| x.parse().ok()).ok_or_else(|| anyhow!("không đo được thời lượng phim"))?;
    let hinh = v["streams"].as_array().is_some_and(|a| a.iter().any(|s| s["codec_type"] == "video"));
    Ok((giay, hinh))
}

const SR: u32 = 48000;

/// File WAV mono 16 bit dài cố định, ghi chèn từng đoạn vào đúng vị trí (không giữ cả phim trong RAM).
struct WavDai {
    f: std::fs::File,
}

impl WavDai {
    fn tao(p: &Path, so_mau: u64) -> Result<Self> {
        let mut f = std::fs::File::create(p)?;
        let n = (so_mau * 2) as u32;
        f.write_all(b"RIFF")?;
        f.write_all(&(36 + n).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&[1, 0, 1, 0])?;
        f.write_all(&SR.to_le_bytes())?;
        f.write_all(&(SR * 2).to_le_bytes())?;
        f.write_all(&[2, 0, 16, 0])?;
        f.write_all(b"data")?;
        f.write_all(&n.to_le_bytes())?;
        f.set_len(44 + n as u64)?;
        Ok(Self { f })
    }

    fn ghi_tai(&mut self, vt: u64, x: &[f32]) -> Result<()> {
        self.f.seek(SeekFrom::Start(44 + vt * 2))?;
        let mut b = Vec::with_capacity(x.len() * 2);
        for v in x {
            b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
        }
        self.f.write_all(&b)?;
        Ok(())
    }
}

pub struct KetQua {
    pub phim_moi: PathBuf,
    pub giong: PathBuf,
    pub so_dong: usize,
    pub bo_qua: usize,
}

fn id_dong(g: &str, cx: &str, chu: &str, t: &Tuy) -> String {
    let k = format!("{g}|{cx}|{chu}|{}|{}|{:.2}", t.theo_cam_xuc, t.nhan_nha, t.cuong_do);
    format!("{:x}", md5::compute(k.as_bytes()))[..12].to_string()
}

/// Thuyết minh cho `phim` theo các dòng (bắt đầu, kết thúc, câu tiếng Việt; rỗng = bỏ qua).
/// `tm`: thư mục làm việc của phim. → None nếu bị dừng.
pub fn thuyet_minh(
    phim: &Path,
    dong: &[(f64, f64, String)],
    tm: &Path,
    t: &Tuy,
    dung: &AtomicBool,
    bao: &dyn Fn(&str),
    tien: &dyn Fn(usize, usize),
) -> Result<Option<KetQua>> {
    if gv::tim_vieneu().is_none() {
        bail!("không tìm thấy VieNeu (bản GPU) – mở VieNeu, cài 'GPU' hoặc chạy Cài đặt > Tải công cụ");
    }
    if !ffbin("ffmpeg.exe").is_file() {
        bail!("thiếu ffmpeg trong cong_cu");
    }
    // 1. lọc dòng có tiếng Việt, tính khung thời gian mỗi dòng (đến lúc dòng sau bắt đầu)
    let mut dl: Vec<(f64, f64, String)> = dong.iter().filter(|d| !lam_sach_loi(&d.2).is_empty()).cloned().collect();
    let bo_qua = dong.len() - dl.len();
    if dl.is_empty() {
        bail!("chưa có dòng phụ đề tiếng Việt nào – tạo phụ đề trước");
    }
    if let Some(n) = t.thu_dau {
        dl.truncate(n);
    }
    // 2. bộ giọng cảm xúc
    if t.theo_cam_xuc && !co_bo(&t.giong) {
        bao(&format!("Tạo bộ giọng cảm xúc cho {} (lần đầu, ~1 phút)...", t.giong));
        if !tao_bo(&[&t.giong], dung, bao)? {
            return Ok(None);
        }
    }
    // 3. VieNeu đọc từng dòng
    let wav_dir = tm.join("thuyet_minh").join("wav");
    std::fs::create_dir_all(&wav_dir)?;
    let mut ds = Vec::new();
    let mut ids = Vec::new();
    for (i, (a, _, vi)) in dl.iter().enumerate() {
        let cx = phan_loai(vi);
        let chu = phien_am::doc_duoc(&lam_sach_loi(vi));
        let khung = dl.get(i + 1).map(|n| n.0 - a - 0.05).unwrap_or(8.0).clamp(0.6, 12.0);
        let id = id_dong(&t.giong, cx, &chu, t);
        ds.push(json!({"id": id, "text": chu, "giay": khung, "cam_xuc": cx}));
        ids.push(id);
    }
    let viec = json!({
        "che_do": "doc", "giong": t.giong, "ra": wav_dir.to_string_lossy(), "ds": ds,
        "bo": if t.theo_cam_xuc { Some(thu_muc_giong(&t.giong).to_string_lossy().into_owned()) } else { None },
        "nhan_nha": t.nhan_nha, "cuong_do": t.cuong_do, "toi_da": 1.35,
    });
    let n = dl.len();
    bao(&format!("VieNeu đang đọc {n} dòng bằng giọng {}...", t.giong));
    if !gv::chay_kich_ban("thuyet_minh.py", &viec, &tm.join("thuyet_minh"), "Thuyết minh: VieNeu đã đọc", dung, &|s| {
        bao(s);
        if let Some((a, b)) = s.rsplit_once(' ').and_then(|x| x.1.split_once('/')) {
            tien(a.parse().unwrap_or(0), b.parse().unwrap_or(1));
        }
    })? {
        return Ok(None);
    }
    // 4. xếp các đoạn vào đúng thời điểm
    bao("Đang xếp giọng thuyết minh vào đúng thời điểm...");
    let (dai, co_hinh) = thong_tin(phim)?;
    let cuoi = dl.last().map(|d| d.0).unwrap_or(0.0) + 15.0;
    let tong = (dai.max(cuoi) * SR as f64) as u64;
    let giong_wav = tm.join("thuyet_minh").join("giong.wav");
    let mut w = WavDai::tao(&giong_wav, tong)?;
    let mut da = 0;
    for (i, (a, _, _)) in dl.iter().enumerate() {
        let p = wav_dir.join(format!("{}.wav", ids[i]));
        let Ok(b) = std::fs::read(&p) else { continue };
        let Ok((x, ts)) = wwise::doc_wav(&b) else { continue };
        let mut x = if ts == SR { x } else { wwise::chuan_hoa(&x, ts, 0.0) };
        let vt = (a * SR as f64) as u64;
        // không đè lên dòng sau: cắt ở lúc dòng sau bắt đầu (mờ dần 30 ms)
        if let Some(n) = dl.get(i + 1) {
            let toi_da = ((n.0 - a) * SR as f64) as usize;
            if x.len() > toi_da && toi_da > 1500 {
                x.truncate(toi_da);
                let m = 1440.min(x.len());
                let l = x.len();
                for k in 0..m {
                    x[l - m + k] *= 1.0 - k as f32 / m as f32;
                }
            }
        }
        w.ghi_tai(vt, &x)?;
        da += 1;
        if dung.load(Ordering::Relaxed) {
            return Ok(None);
        }
    }
    drop(w);
    // 5. trộn: hạ tiếng gốc khi thuyết minh nói (sidechain), giữ hình
    bao("Đang trộn giọng thuyết minh vào phim (ffmpeg)...");
    let ten = phim.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let thu_ra = phim.parent().filter(|p| std::fs::metadata(p).map(|m| !m.permissions().readonly()).unwrap_or(false)).unwrap_or(tm);
    let phim_moi = thu_ra.join(format!("{ten}_thuyetminh.{}", if co_hinh { "mp4" } else { "m4a" }));
    let giong_m4a = thu_ra.join(format!("{ten}_thuyetminh_giong.m4a"));
    let loc = "[1:a]asplit=2[sc][n];[0:a][sc]sidechaincompress=threshold=0.02:ratio=9:attack=15:release=450[bg];[bg][n]amix=inputs=2:duration=first:normalize=0[m]";
    let mut c = Command::new(ffbin("ffmpeg.exe"));
    an(&mut c).args(["-y", "-v", "error", "-i"]).arg(phim).arg("-i").arg(&giong_wav).args(["-filter_complex", loc]);
    if co_hinh {
        c.args(["-map", "0:v", "-c:v", "copy"]);
    }
    c.args(["-map", "[m]", "-c:a", "aac", "-b:a", "192k"]).arg(&phim_moi).stdout(Stdio::null()).stderr(Stdio::piped());
    let mut con = c.spawn()?;
    loop {
        if dung.load(Ordering::Relaxed) {
            let _ = con.kill();
            let _ = con.wait();
            return Ok(None);
        }
        if let Some(st) = con.try_wait()? {
            if !st.success() {
                let mut e = String::new();
                if let Some(mut s) = con.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut s, &mut e);
                }
                bail!("ffmpeg trộn lỗi: {}", e.lines().last().unwrap_or(""));
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
    }
    // file giọng riêng (nhỏ) để dùng với trình phát khác / làm tư liệu
    let mut c2 = Command::new(ffbin("ffmpeg.exe"));
    an(&mut c2).args(["-y", "-v", "error", "-i"]).arg(&giong_wav).args(["-t", &format!("{dai:.2}"), "-c:a", "aac", "-b:a", "96k"]).arg(&giong_m4a).stdout(Stdio::null()).stderr(Stdio::null());
    let _ = c2.status();
    let _ = std::fs::remove_file(&giong_wav);
    Ok(Some(KetQua { phim_moi, giong: giong_m4a, so_dong: da, bo_qua }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn phan_loai_dung() {
        assert_eq!(phan_loai("Chạy mau! Chúng đang đuổi theo!"), "gap_rut");
        assert_eq!(phan_loai("Được rồi!"), "nhan_manh");
        assert_eq!(phan_loai("Anh ấy lặng lẽ cúi đầu..."), "buon");
        assert_eq!(phan_loai("Bạn đi đâu vậy?"), "hoi");
        assert_eq!(phan_loai("Thành phố này rất yên bình."), "binh_thuong");
        assert_eq!(phan_loai("Không! Không! Đừng!"), "gap_rut");
    }

    #[test]
    fn id_theo_noi_dung() {
        let t = Tuy { giong: "Thiện Minh".into(), theo_cam_xuc: true, nhan_nha: true, cuong_do: 0.7, thu_dau: None };
        assert_ne!(id_dong("a", "buon", "x", &t), id_dong("a", "gap_rut", "x", &t));
        assert_eq!(id_dong("a", "buon", "x", &t).len(), 12);
    }

    struct Bc;
    impl crate::viet_hoa::viec_dich::BaoCao for Bc {
        fn bao(&self, s: &str) {
            println!("{s}");
        }
        fn tien_do(&self, _: usize, _: usize, _: f64) {}
    }

    /// Cả chuỗi trên đoạn phim thật: FILE=<video> cargo test --release -- --ignored thuyet_minh_that --nocapture
    #[test]
    #[ignore]
    fn thuyet_minh_that() {
        use crate::phu_de_phim::ViecPhim;
        let f = std::env::var("FILE").unwrap();
        let dung = AtomicBool::new(false);
        let mut v = ViecPhim::moi(&f).unwrap();
        v.tai_va_nghe(None, &dung, &|s| println!("{s}"), &|_, _, _| {}).unwrap();
        let ds = v.cau_can_dich();
        v.viec.chay_offline(ds, &Bc).unwrap();
        let dong: Vec<(f64, f64, String)> = v.dong.iter().map(|(a, b, c)| (*a, *b, v.viec.tim(c).unwrap_or_default())).collect();
        for d in &dong {
            println!("{:.1} [{}] {}", d.0, phan_loai(&d.2), d.2);
        }
        let t = Tuy { giong: "Thiện Minh".into(), theo_cam_xuc: true, nhan_nha: true, cuong_do: 0.7, thu_dau: None };
        let kq = thuyet_minh(v.phim.as_ref().map(|p| p.duong_dan.as_path()).unwrap(), &dong, &v.thu_muc_phim(), &t, &dung, &|s| println!("{s}"), &|a, b| println!("{a}/{b}"))
            .unwrap()
            .unwrap();
        println!("{} dòng • {:?} • {:?}", kq.so_dong, kq.phim_moi, kq.giong);
        assert!(std::fs::metadata(&kq.phim_moi).unwrap().len() > 10_000);
    }
}
