//! Lồng tiếng CHUNG cho game bất kỳ có file thoại rời (wav / ogg / mp3 / flac / opus):
//! quét file âm thanh → ghép với câu phụ đề đã dịch (theo tên file ↔ khóa chữ, hoặc bảng CSV tự sửa) →
//! nhóm theo người nói → giọng mẫu (kho giọng riêng hoặc nhái giọng gốc) → VieNeu → mã hóa lại đúng định dạng
//! gốc (ffmpeg) → gói `LongTieng\` + cài / gỡ có sao lưu. Game dùng gói riêng (Wwise .wem, FSB, .bank...) cần
//! công cụ riêng như Crimson Desert / Tomb Raider.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use super::cd_giong::lam_sach_loi;
use super::{giong_vieneu as gv, phien_am, wwise};
use crate::duong_dan;

pub const DUOI: [&str; 5] = ["wav", "ogg", "mp3", "flac", "opus"];

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Clip {
    /// đường dẫn tương đối trong thư mục game (dấu /)
    pub rel: String,
    pub giay: f64,
    pub sr: u32,
    pub kenh: u32,
    pub nguoi: String,
    pub en: Option<String>,
    pub vi: Option<String>,
}

fn ffmpeg_bin(ten: &str) -> PathBuf {
    duong_dan::cong_cu().join("ffmpeg").join("bin").join(ten)
}

fn an(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c
}

fn id_clip(rel: &str) -> String {
    format!("{:x}", md5::compute(rel.as_bytes()))[..12].to_string()
}

/// (giây, tần số, số kênh) của file âm thanh.
fn do_thong_tin(p: &Path) -> Option<(f64, u32, u32)> {
    let mut c = Command::new(ffmpeg_bin("ffprobe.exe"));
    an(&mut c)
        .args(["-v", "error", "-show_entries", "format=duration:stream=sample_rate,channels", "-of", "json"])
        .arg(p)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let o = c.output().ok()?;
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).ok()?;
    let giay = v["format"]["duration"].as_str()?.parse::<f64>().ok()?;
    let s = v["streams"].get(0)?;
    let sr = s["sample_rate"].as_str().and_then(|x| x.parse().ok()).unwrap_or(44100);
    let kenh = s["channels"].as_u64().unwrap_or(1) as u32;
    Some((giay, sr, kenh))
}

fn duoi_hop_le(p: &Path) -> bool {
    p.extension().is_some_and(|e| DUOI.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

/// Quét file âm thanh rời của game (bỏ file < 3 KB hoặc > 40 MB: tiếng động / nhạc nền dài).
pub fn quet(thu_muc: &Path, bao: &(dyn Fn(&str) + Sync), dung: &AtomicBool) -> Vec<Clip> {
    let mut ds: Vec<PathBuf> = Vec::new();
    let mut cho = vec![thu_muc.to_path_buf()];
    while let Some(d) = cho.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                cho.push(p);
            } else if duoi_hop_le(&p) {
                let kich = e.metadata().map(|m| m.len()).unwrap_or(0);
                if (3_000..40_000_000).contains(&kich) {
                    ds.push(p);
                }
            }
        }
    }
    ds.sort();
    bao(&format!("Tìm thấy {} file âm thanh – đang đo thời lượng...", ds.len()));
    let xong = AtomicUsize::new(0);
    let kq: std::sync::Mutex<Vec<Clip>> = Default::default();
    let tiep = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| loop {
                let i = tiep.fetch_add(1, Ordering::Relaxed);
                if i >= ds.len() || dung.load(Ordering::Relaxed) {
                    return;
                }
                if let Some((giay, sr, kenh)) = do_thong_tin(&ds[i]) {
                    let rel = ds[i].strip_prefix(thu_muc).unwrap_or(&ds[i]).to_string_lossy().replace('\\', "/");
                    kq.lock().unwrap().push(Clip { rel, giay, sr, kenh, ..Default::default() });
                }
                let n = xong.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 500 == 0 {
                    bao(&format!("Đang đo thời lượng {n}/{}", ds.len()));
                }
            });
        }
    });
    let mut v = kq.into_inner().unwrap();
    v.sort_by(|a, b| a.rel.cmp(&b.rel));
    gan_nguoi_noi(&mut v);
    v
}

fn stem(rel: &str) -> String {
    let t = rel.rsplit('/').next().unwrap_or(rel);
    t.rsplit_once('.').map(|x| x.0).unwrap_or(t).to_lowercase()
}

/// Tiền tố tên file (trước số / mã): "lara_0123" → "lara".
fn tien_to(s: &str) -> String {
    let t: Vec<&str> = s.split(['_', '-', ' ']).filter(|x| !x.is_empty()).collect();
    let i = t.iter().position(|x| x.chars().any(|c| c.is_ascii_digit())).unwrap_or(t.len());
    let k = &t[..i.min(t.len())];
    if k.is_empty() { "khac".into() } else { k.join("_") }
}

/// Người nói: nếu thư mục chứa nhiều tiền tố tên file khác nhau thì theo tiền tố, không thì theo tên thư mục.
fn gan_nguoi_noi(v: &mut [Clip]) {
    let mut theo_thu_muc: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, c) in v.iter().enumerate() {
        let d = c.rel.rsplit_once('/').map(|x| x.0).unwrap_or("").to_string();
        theo_thu_muc.entry(d).or_default().push(i);
    }
    for (d, ids) in theo_thu_muc {
        let tt: std::collections::HashSet<String> = ids.iter().map(|&i| tien_to(&stem(&v[i].rel))).collect();
        let theo_tien_to = tt.len() >= 3 && tt.len() * 2 <= ids.len();
        let ten_d = d.rsplit('/').next().filter(|x| !x.is_empty()).unwrap_or("khac").to_lowercase();
        for i in ids {
            v[i].nguoi = if theo_tien_to { tien_to(&stem(&v[i].rel)) } else { ten_d.clone() };
        }
    }
}

/// Ghép câu: khóa chữ ↔ tên file (khớp đúng, hoặc phần đuôi tên file). `chu`: khóa → chữ Anh;
/// `tim_viet`: tra bản dịch của câu Anh.
pub fn ghep_chu(clips: &mut [Clip], chu: &BTreeMap<String, Vec<(String, String)>>, tim_viet: &dyn Fn(&str) -> Option<String>) -> usize {
    let mut khoa: HashMap<String, &str> = HashMap::new();
    for ds in chu.values() {
        for (k, en) in ds {
            let kn = k.rsplit(['/', '\\']).next().unwrap_or(k).to_lowercase();
            if en.chars().any(char::is_alphabetic) {
                khoa.entry(kn).or_insert(en.as_str());
            }
        }
    }
    let mut dem = 0;
    for c in clips.iter_mut() {
        let t = stem(&c.rel);
        let phan: Vec<&str> = t.split('_').collect();
        let en = (0..phan.len()).find_map(|j| {
            let duoi = phan[j..].join("_");
            khoa.get(&duoi).copied()
        });
        if let Some(en) = en {
            c.en = Some(en.to_string());
            c.vi = tim_viet(en).filter(|v| v != en && !v.is_empty());
            dem += 1;
        }
    }
    dem
}

/// Bảng ghép CSV (người dùng tự sửa trong Excel): tep, nguoi, tieng_anh, tieng_viet.
pub fn xuat_csv(clips: &[Clip], p: &Path) -> Result<usize> {
    let mut w = csv::Writer::from_path(p)?;
    w.write_record(["tep", "nguoi", "giay", "tieng_anh", "tieng_viet"])?;
    for c in clips {
        w.write_record([&c.rel, &c.nguoi, &format!("{:.2}", c.giay), c.en.as_deref().unwrap_or(""), c.vi.as_deref().unwrap_or("")])?;
    }
    w.flush()?;
    Ok(clips.len())
}

pub fn nap_csv(clips: &mut [Clip], p: &Path) -> Result<usize> {
    let mut r = csv::ReaderBuilder::new().flexible(true).from_path(p)?;
    let theo_rel: HashMap<String, usize> = clips.iter().enumerate().map(|(i, c)| (c.rel.clone(), i)).collect();
    let mut dem = 0;
    for b in r.records().flatten() {
        let (Some(rel), Some(i)) = (b.get(0), b.get(0).and_then(|r| theo_rel.get(r))) else { continue };
        let _ = rel;
        let c = &mut clips[*i];
        if let Some(n) = b.get(1).filter(|x| !x.trim().is_empty()) {
            c.nguoi = n.trim().to_lowercase();
        }
        c.en = b.get(3).filter(|x| !x.trim().is_empty()).map(String::from);
        c.vi = b.get(4).filter(|x| !x.trim().is_empty()).map(String::from);
        dem += 1;
    }
    Ok(dem)
}

/// Giải mã đoạn file âm thanh → WAV 48 kHz mono (f32).
fn giai_ma(p: &Path, tam: &Path) -> Option<Vec<f32>> {
    let ra = tam.join("dec.wav");
    let _ = std::fs::remove_file(&ra);
    let mut c = Command::new(ffmpeg_bin("ffmpeg.exe"));
    an(&mut c)
        .args(["-y", "-v", "error", "-i"])
        .arg(p)
        .args(["-vn", "-ac", "1", "-ar", "48000", "-c:a", "pcm_s16le"])
        .arg(&ra)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if !c.status().ok()?.success() {
        return None;
    }
    wwise::doc_wav(&std::fs::read(&ra).ok()?).ok().map(|x| x.0)
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        0.0
    } else {
        (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
    }
}

/// Giọng mẫu ~9–15 giây ghép từ chính các câu gốc của người nói (3–9 s trước, rồi 1,2–3 s / 9–14 s).
fn mau_tu_goc(game: &Path, cau: &[&Clip], ra: &Path, tam: &Path) -> Result<bool> {
    std::fs::create_dir_all(tam)?;
    let mut uu: Vec<&&Clip> = cau.iter().filter(|c| (3.0..=9.0).contains(&c.giay)).collect();
    uu.sort_by(|a, b| b.giay.total_cmp(&a.giay));
    let mut phu: Vec<&&Clip> = cau.iter().filter(|c| (1.2..3.0).contains(&c.giay) || (9.0 < c.giay && c.giay <= 14.0)).collect();
    phu.sort_by(|a, b| b.giay.total_cmp(&a.giay));
    uu.extend(phu);
    let (mut y, mut tong) = (Vec::<f32>::new(), 0.0);
    for c in uu.into_iter().take(16) {
        let Some(x) = giai_ma(&game.join(&c.rel), tam) else { continue };
        if x.len() < 48000 || rms(&x) < 0.01 {
            continue;
        }
        tong += x.len() as f64 / 48000.0;
        y.extend(x);
        y.extend(vec![0f32; 14400]);
        if tong >= 9.0 {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(tam);
    if y.is_empty() {
        return Ok(false);
    }
    y.truncate(15 * 48000);
    wwise::ghi_wav16(ra, &y, 48000)?;
    Ok(true)
}

/// Mã hóa WAV về đúng định dạng / tần số / số kênh của file gốc.
fn ma_hoa_lai(wav: &Path, c: &Clip, ra: &Path) -> bool {
    let duoi = c.rel.rsplit('.').next().unwrap_or("wav").to_lowercase();
    let codec: &[&str] = match duoi.as_str() {
        "ogg" => &["-c:a", "libvorbis", "-q:a", "5"],
        "mp3" => &["-c:a", "libmp3lame", "-q:a", "2"],
        "flac" => &["-c:a", "flac"],
        "opus" => &["-c:a", "libopus", "-b:a", "96k"],
        _ => &["-c:a", "pcm_s16le"],
    };
    if let Some(cha) = ra.parent() {
        let _ = std::fs::create_dir_all(cha);
    }
    let tam = ra.with_extension(format!("{duoi}.tam.{duoi}"));
    let mut l = Command::new(ffmpeg_bin("ffmpeg.exe"));
    an(&mut l)
        .args(["-y", "-v", "error", "-i"])
        .arg(wav)
        .args(["-ar", &c.sr.to_string(), "-ac", &c.kenh.clamp(1, 2).to_string()])
        .args(codec)
        .arg(&tam)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let ok = l.status().is_ok_and(|s| s.success());
    ok && std::fs::rename(&tam, ra).is_ok()
}

/// Thư mục làm việc / đầu ra của một game: `<lv>\Chung\<tên>`.
pub fn thu_muc_lv(lv_goc: &Path, ten_game: &str) -> PathBuf {
    let ten: String = ten_game.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    lv_goc.join("Chung").join(ten)
}

pub struct KetQua {
    pub da_lam: usize,
    pub nguoi: usize,
    pub thieu_mau: Vec<String>,
}

/// Lồng tiếng các clip đã có bản dịch. `gan`: người nói → file giọng trong kho giọng (không có → nhái giọng gốc).
/// File mới ghi vào `<lv>\LongTieng\<đường dẫn tương đối>`. → None nếu bị dừng.
pub fn long_tieng(
    game: &Path,
    lv: &Path,
    clips: &[Clip],
    gan: &HashMap<String, PathBuf>,
    dung: &AtomicBool,
    bao: &dyn Fn(&str),
    tien_do: &(dyn Fn(usize, usize, &str) + Sync),
) -> Result<Option<KetQua>> {
    if gv::tim_vieneu().is_none() {
        bail!("không tìm thấy app VieNeu (bản tăng tốc GPU) – mở VieNeu, cài 'GPU' rồi thử lại");
    }
    if !ffmpeg_bin("ffmpeg.exe").is_file() {
        bail!("thiếu ffmpeg trong cong_cu");
    }
    let can: Vec<&Clip> = clips.iter().filter(|c| c.vi.as_deref().is_some_and(|v| !lam_sach_loi(v).is_empty())).collect();
    if can.is_empty() {
        bail!("chưa có câu thoại nào được ghép chữ + dịch – ghép chữ (hoặc nạp CSV) trước");
    }
    let mut theo_nguoi: BTreeMap<String, Vec<&Clip>> = BTreeMap::new();
    for c in &can {
        theo_nguoi.entry(c.nguoi.clone()).or_default().push(c);
    }
    let tam = lv.join("tam");
    let mut viec = Vec::new();
    let mut thieu = Vec::new();
    let mut ds_nguoi: Vec<(&String, &Vec<&Clip>)> = theo_nguoi.iter().collect();
    ds_nguoi.sort_by_key(|(_, d)| std::cmp::Reverse(d.len()));
    for (i, (ng, ds)) in ds_nguoi.iter().enumerate() {
        bao(&format!("Bước 1/3: giọng mẫu {}/{}: {ng}", i + 1, ds_nguoi.len()));
        let thu = lv.join("giong").join(ng.as_str());
        std::fs::create_dir_all(&thu)?;
        let mau = thu.join("mau.wav");
        if let Some(k) = gan.get(ng.as_str()).filter(|p| p.is_file()) {
            std::fs::copy(k, &mau)?;
        } else if !mau.is_file() {
            // nhái giọng gốc của chính người nói này (chọn từ mọi clip có thời lượng hợp lý)
            let moi: Vec<&Clip> = clips.iter().filter(|c| &c.nguoi == *ng).collect();
            if !mau_tu_goc(game, &moi, &mau, &tam.join("mau"))? {
                thieu.push(ng.to_string());
                continue;
            }
        }
        let r = std::fs::read(&mau).ok().and_then(|b| wwise::doc_wav(&b).ok()).map(|(x, _)| rms(&x)).filter(|r| *r > 0.0).unwrap_or(0.08);
        viec.push(gv::Nhom {
            ten: ng.to_string(),
            mau,
            ra: thu.join("wav"),
            rms: r.clamp(0.03, 0.2),
            ds: ds
                .iter()
                .map(|c| (id_clip(&c.rel), phien_am::doc_duoc(&lam_sach_loi(c.vi.as_deref().unwrap_or(""))), (c.giay * 1000.0).round() / 1000.0))
                .collect(),
        });
        if dung.load(Ordering::Relaxed) {
            return Ok(None);
        }
    }
    if !viec.is_empty() && !gv::chay(&viec, &tam, 1.4, dung, &|a, b, s| tien_do(a, b, &format!("Bước 2/3: VieNeu đã đọc{s}")))? {
        return Ok(None);
    }
    bao("Bước 3/3: mã hóa lại đúng định dạng gốc...");
    let ra_dir = lv.join("LongTieng");
    let mut viec_enc: Vec<(&Clip, PathBuf)> = Vec::new();
    for v in &viec {
        for d in &v.ds {
            let wav = v.ra.join(format!("{}.wav", d.0));
            if let Some(c) = can.iter().find(|c| id_clip(&c.rel) == d.0) {
                if wav.is_file() {
                    viec_enc.push((c, wav));
                }
            }
        }
    }
    let xong = AtomicUsize::new(0);
    let tiep = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..6 {
            s.spawn(|| loop {
                let i = tiep.fetch_add(1, Ordering::Relaxed);
                if i >= viec_enc.len() || dung.load(Ordering::Relaxed) {
                    return;
                }
                let (c, wav) = &viec_enc[i];
                let ra = ra_dir.join(&c.rel);
                let moi = ra.is_file()
                    && ra.metadata().and_then(|m| m.modified()).ok() >= wav.metadata().and_then(|m| m.modified()).ok();
                if moi || ma_hoa_lai(wav, c, &ra) {
                    let n = xong.fetch_add(1, Ordering::Relaxed) + 1;
                    if n % 200 == 0 {
                        tien_do(n, viec_enc.len(), " (mã hóa)");
                    }
                }
            });
        }
    });
    if dung.load(Ordering::Relaxed) {
        return Ok(None);
    }
    Ok(Some(KetQua { da_lam: xong.load(Ordering::Relaxed), nguoi: viec.len(), thieu_mau: thieu }))
}

/// Chép file lồng tiếng vào game, sao lưu bản gốc lần đầu vào `<lv>\sao_luu_goc`. → số file.
pub fn cai(game: &Path, lv: &Path) -> Result<usize> {
    let nguon = lv.join("LongTieng");
    let sao = lv.join("sao_luu_goc");
    if !nguon.is_dir() {
        bail!("chưa có file lồng tiếng – bấm Lồng tiếng trước");
    }
    let mut cho = vec![nguon.clone()];
    let mut n = 0;
    while let Some(d) = cho.pop() {
        for e in std::fs::read_dir(&d)?.flatten() {
            let p = e.path();
            if p.is_dir() {
                cho.push(p);
                continue;
            }
            let rel = p.strip_prefix(&nguon).map_err(|e| anyhow!("{e}"))?.to_path_buf();
            let dich = game.join(&rel);
            let s = sao.join(&rel);
            if dich.is_file() && !s.exists() {
                std::fs::create_dir_all(s.parent().unwrap())?;
                std::fs::copy(&dich, &s)?;
            }
            std::fs::create_dir_all(dich.parent().unwrap())?;
            std::fs::copy(&p, &dich)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Trả lại file gốc từ bản sao lưu. → số file.
pub fn go(game: &Path, lv: &Path) -> Result<usize> {
    let sao = lv.join("sao_luu_goc");
    if !sao.is_dir() {
        bail!("không có bản sao lưu để trả lại");
    }
    let mut cho = vec![sao.clone()];
    let mut n = 0;
    while let Some(d) = cho.pop() {
        for e in std::fs::read_dir(&d)?.flatten() {
            let p = e.path();
            if p.is_dir() {
                cho.push(p);
                continue;
            }
            let rel = p.strip_prefix(&sao).map_err(|e| anyhow!("{e}"))?;
            std::fs::copy(&p, game.join(rel))?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn nguoi_noi_va_ghep() {
        let mut v: Vec<Clip> = ["vo/lara_001.ogg", "vo/lara_002.ogg", "vo/roth_001.ogg", "vo/roth_002.ogg", "vo/sam_001.ogg", "vo/sam_002.ogg", "vo/jonah_001.ogg", "vo/jonah_002.ogg", "sfx/boom_1.wav"]
            .iter()
            .map(|r| Clip { rel: r.to_string(), giay: 2.0, sr: 44100, kenh: 1, ..Default::default() })
            .collect();
        gan_nguoi_noi(&mut v);
        assert_eq!(v[0].nguoi, "lara");
        assert_eq!(v[1].nguoi, "lara");
        assert_eq!(v[2].nguoi, "roth");
        assert_eq!(v[8].nguoi, "sfx");
        let mut chu = BTreeMap::new();
        chu.insert("f".to_string(), vec![("lara_001".to_string(), "Hello there friend".to_string()), ("VO/roth_002".to_string(), "Run now".to_string())]);
        let n = ghep_chu(&mut v, &chu, &|en| Some(format!("VI:{en}")));
        assert_eq!(n, 2);
        assert_eq!(v[0].vi.as_deref(), Some("VI:Hello there friend"));
        assert_eq!(v[3].vi.as_deref(), Some("VI:Run now"));
        assert_eq!(id_clip("a").len(), 12);
    }

    #[test]
    fn csv_khu_hoi() {
        let mut v = vec![Clip { rel: "a/b.ogg".into(), giay: 1.5, nguoi: "x".into(), en: Some("Hi, you".into()), vi: Some("Chào".into()), ..Default::default() }];
        let p = std::env::temp_dir().join("vh_lt_chung.csv");
        xuat_csv(&v, &p).unwrap();
        v[0].vi = None;
        v[0].nguoi = "y".into();
        assert_eq!(nap_csv(&mut v, &p).unwrap(), 1);
        assert_eq!(v[0].vi.as_deref(), Some("Chào"));
        assert_eq!(v[0].nguoi, "x");
    }

    /// Cả chuỗi trên game giả (3 file ogg lấy từ giọng VieNeu thử): quét → ghép → lồng tiếng → cài → gỡ.
    /// MAU=<wav giọng nói dài >3s> cargo test --release -- --ignored chuoi_chung_that --nocapture
    #[test]
    #[ignore]
    fn chuoi_chung_that() {
        let mau = PathBuf::from(std::env::var("MAU").unwrap());
        let tm = std::env::temp_dir().join("vh_lt_chung_game");
        let lv = std::env::temp_dir().join("vh_lt_chung_lv");
        let _ = std::fs::remove_dir_all(&tm);
        let _ = std::fs::remove_dir_all(&lv);
        std::fs::create_dir_all(tm.join("vo")).unwrap();
        for i in 1..=3 {
            let mut c = Command::new(ffmpeg_bin("ffmpeg.exe"));
            an(&mut c).args(["-y", "-v", "error", "-i"]).arg(&mau).args(["-ar", "44100", "-ac", "1", "-c:a", "libvorbis"]).arg(tm.join("vo").join(format!("kliff_00{i}.ogg")));
            assert!(c.status().unwrap().success());
        }
        let dung = AtomicBool::new(false);
        let mut clips = quet(&tm, &|s| println!("{s}"), &dung);
        assert_eq!(clips.len(), 3);
        println!("người nói: {:?}, {:.1}s", clips[0].nguoi, clips[0].giay);
        let mut chu = BTreeMap::new();
        chu.insert("f".to_string(), vec![
            ("kliff_001".to_string(), "Hello there, my friend.".to_string()),
            ("kliff_002".to_string(), "We must leave now!".to_string()),
            ("kliff_003".to_string(), "Be careful of the enemy ahead.".to_string()),
        ]);
        let vi: HashMap<&str, &str> = [
            ("Hello there, my friend.", "Xin chào, bạn của tôi."),
            ("We must leave now!", "Chúng ta phải đi ngay!"),
            ("Be careful of the enemy ahead.", "Hãy cẩn thận kẻ thù phía trước."),
        ].into_iter().collect();
        assert_eq!(ghep_chu(&mut clips, &chu, &|en| vi.get(en).map(|s| s.to_string())), 3);
        let kq = long_tieng(&tm, &lv, &clips, &HashMap::new(), &dung, &|s| println!("{s}"), &|a, b, s| println!("{a}/{b}{s}")).unwrap().unwrap();
        println!("đã làm {} file, {} người, thiếu mẫu {:?}", kq.da_lam, kq.nguoi, kq.thieu_mau);
        assert_eq!(kq.da_lam, 3);
        let goc: Vec<Vec<u8>> = clips.iter().map(|c| std::fs::read(tm.join(&c.rel)).unwrap()).collect();
        assert_eq!(cai(&tm, &lv).unwrap(), 3);
        let moi: Vec<Vec<u8>> = clips.iter().map(|c| std::fs::read(tm.join(&c.rel)).unwrap()).collect();
        assert!(goc.iter().zip(&moi).all(|(a, b)| a != b), "file phải đổi sau khi cài");
        assert_eq!(&moi[0][..4], b"OggS");
        assert_eq!(go(&tm, &lv).unwrap(), 3);
        let lai: Vec<Vec<u8>> = clips.iter().map(|c| std::fs::read(tm.join(&c.rel)).unwrap()).collect();
        assert_eq!(goc, lai, "gỡ phải trả lại đúng từng byte");
    }
}

/// Đếm file âm thanh đóng gói (Wwise, FMOD, CRI...) trong thư mục game để báo lý do rõ ràng.
pub fn dem_goi_am_thanh(thu_muc: &Path) -> Vec<(String, usize)> {
    const GOI: [&str; 12] = ["wem", "bnk", "bank", "fsb", "pck", "awb", "acb", "xwb", "xsb", "snd", "sdt", "fsp"];
    let mut dem: std::collections::BTreeMap<String, usize> = Default::default();
    let mut cho = vec![thu_muc.to_path_buf()];
    let mut so = 0usize;
    while let Some(d) = cho.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                cho.push(p);
            } else if let Some(x) = p.extension().map(|x| x.to_string_lossy().to_lowercase()) {
                if GOI.contains(&x.as_str()) {
                    *dem.entry(x).or_default() += 1;
                }
            }
            so += 1;
            if so > 400_000 {
                return dem.into_iter().collect();
            }
        }
    }
    dem.into_iter().collect()
}
