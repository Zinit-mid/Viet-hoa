//! Mã hóa giọng Việt (WAV) thành .wem bằng WwiseConsole và đóng gói zip cho DMM (Crimson Desert).
//! Chuyển từ `tim_wwise_console / chuan_wav / ma_hoa_wwise` của app cũ.
//! Chia lô (mặc định 3.000 file) để đỡ tốn ổ đĩa; chạy lại thì bỏ qua câu đã có .wem.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, Context, Result};

pub const TAN_SO: u32 = 48000;

/// WwiseConsole.exe: biến WWISEROOT, rồi các thư mục cài mặc định trên mọi ổ.
pub fn tim_wwise_console() -> Option<PathBuf> {
    let phan_duoi = ["Authoring", "x64", "Release", "bin", "WwiseConsole.exe"];
    let mut ung_vien: Vec<PathBuf> = Vec::new();
    if let Some(g) = std::env::var_os("WWISEROOT") {
        ung_vien.push(phan_duoi.iter().fold(PathBuf::from(g), |p, x| p.join(x)));
    }
    for o in "CDEFGH".chars() {
        let goc_ds = [
            format!(r"{o}:\Program Files (x86)\Audiokinetic"),
            format!(r"{o}:\Program Files\Audiokinetic"),
            format!(r"{o}:\Audiokinetic"),
            format!(r"{o}:\"),
        ];
        for g in goc_ds {
            let Ok(rd) = std::fs::read_dir(&g) else { continue };
            let mut con: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            con.sort();
            con.reverse(); // bản mới nhất trước
            for c in con {
                ung_vien.push(phan_duoi.iter().fold(c, |p, x| p.join(x)));
            }
        }
    }
    ung_vien.into_iter().find(|p| p.is_file())
}

/// WAV bất kỳ (PCM 16/24/32, float, nhiều kênh) → mẫu mono f32 và tần số gốc.
pub fn doc_wav(b: &[u8]) -> Result<(Vec<f32>, u32)> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err(anyhow!("không phải file WAV"));
    }
    let (mut o, mut fmt, mut data) = (12usize, None, None);
    while o + 8 <= b.len() {
        let co = u32::from_le_bytes(b[o + 4..o + 8].try_into()?) as usize;
        let het = (o + 8 + co).min(b.len());
        match &b[o..o + 4] {
            b"fmt " => fmt = Some(&b[o + 8..het]),
            b"data" => data = Some(&b[o + 8..het]),
            _ => {}
        }
        o += 8 + co + (co & 1);
    }
    let (fmt, data) = fmt.zip(data).ok_or_else(|| anyhow!("WAV hỏng"))?;
    if fmt.len() < 16 {
        return Err(anyhow!("WAV hỏng"));
    }
    let u16le = |i: usize| u16::from_le_bytes([fmt[i], fmt[i + 1]]);
    let mut ma = u16le(0);
    let kenh = u16le(2).max(1) as usize;
    let ts = u32::from_le_bytes(fmt[4..8].try_into()?);
    let bit = u16le(14);
    if ma == 0xFFFE && fmt.len() >= 26 {
        ma = u16le(24);
    }
    let x: Vec<f32> = match (ma, bit) {
        (3, 32) => data.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect(),
        (_, 16) => data.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0).collect(),
        (_, 24) => data
            .chunks_exact(3)
            .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8388608.0)
            .collect(),
        (_, 32) => data.chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap()) as f32 / 2147483648.0).collect(),
        _ => return Err(anyhow!("WAV {bit} bit chưa hỗ trợ")),
    };
    let mono = if kenh > 1 {
        x.chunks_exact(kenh).map(|f| f.iter().sum::<f32>() / kenh as f32).collect()
    } else {
        x
    };
    Ok((mono, ts))
}

/// Mono, đổi tần số về `tan_so` (nội suy tuyến tính), tăng `tang_db` rồi giới hạn mềm (tanh).
pub fn chuan_hoa(x: &[f32], ts: u32, tang_db: f64) -> Vec<f32> {
    let mut y: Vec<f32> = if ts != TAN_SO && x.len() > 1 {
        let n = (x.len() as f64 * TAN_SO as f64 / ts as f64).round() as usize;
        (0..n)
            .map(|i| {
                let p = i as f64 * (x.len() - 1) as f64 / (n.max(2) - 1) as f64;
                let (a, f) = (p as usize, (p - p.floor()) as f32);
                x[a] + (x[(a + 1).min(x.len() - 1)] - x[a]) * f
            })
            .collect()
    } else {
        x.to_vec()
    };
    if tang_db != 0.0 {
        let g = 10f64.powf(tang_db / 20.0) as f32;
        let t = 0.85f32;
        for v in y.iter_mut() {
            let s = *v * g;
            let a = s.abs();
            *v = if a <= t { s } else { s.signum() * (t + (1.0 - t) * ((a - t) / (1.0 - t)).tanh()) };
        }
    }
    y.iter_mut().for_each(|v| *v = v.clamp(-1.0, 1.0));
    y
}

pub fn ghi_wav16(duong: &Path, x: &[f32], ts: u32) -> Result<()> {
    let n = (x.len() * 2) as u32;
    let mut f = std::io::BufWriter::new(std::fs::File::create(duong)?);
    f.write_all(b"RIFF")?;
    f.write_all(&(36 + n).to_le_bytes())?;
    f.write_all(b"WAVEfmt ")?;
    f.write_all(&16u32.to_le_bytes())?;
    f.write_all(&[1, 0, 1, 0])?;
    f.write_all(&ts.to_le_bytes())?;
    f.write_all(&(ts * 2).to_le_bytes())?;
    f.write_all(&[2, 0, 16, 0])?;
    f.write_all(b"data")?;
    f.write_all(&n.to_le_bytes())?;
    for v in x {
        f.write_all(&((v * 32767.0) as i16).to_le_bytes())?;
    }
    Ok(())
}

fn xml_thoat(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn chay_wwise(wwise: &Path, args: &[&std::ffi::OsStr], nk: &mut impl Write) -> Result<()> {
    let mut lenh = Command::new(wwise);
    lenh.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        lenh.creation_flags(0x0800_0000);
    }
    let r = lenh.output().context("không chạy được WwiseConsole")?;
    let _ = nk.write_all(&r.stdout);
    let _ = nk.write_all(&r.stderr);
    Ok(())
}

/// `muc`: (tên, file wav) → file .wem trong `thu_ra`. Trả {tên: đường dẫn .wem}.
/// Câu đã có .wem trong `thu_ra` thì bỏ qua (làm tiếp). `tam` được dọn khi xong.
pub fn ma_hoa(
    muc: &[(String, PathBuf)],
    wwise: &Path,
    thu_ra: &Path,
    tam: &Path,
    tang_db: f64,
    lo: usize,
    file_nhat_ky: &Path,
    dung: &AtomicBool,
    bao: &dyn Fn(&str),
) -> Result<BTreeMap<String, PathBuf>> {
    let _ = std::fs::remove_dir_all(tam);
    std::fs::create_dir_all(tam)?;
    std::fs::create_dir_all(thu_ra)?;
    let mut nk = std::fs::OpenOptions::new().create(true).append(true).open(file_nhat_ky)?;
    let kq = (|| -> Result<BTreeMap<String, PathBuf>> {
        let du_an = tam.join("VH").join("VH.wproj");
        chay_wwise(wwise, &["create-new-project".as_ref(), du_an.as_os_str(), "--platform".as_ref(), "Windows".as_ref(), "--quiet".as_ref()], &mut nk)?;
        if !du_an.is_file() {
            return Err(anyhow!("Wwise không tạo được project tạm – xem {}", file_nhat_ky.display()));
        }
        let mut co = BTreeMap::new();
        for e in std::fs::read_dir(thu_ra)?.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wem")) {
                if let Some(t) = p.file_stem() {
                    co.insert(t.to_string_lossy().into_owned(), p);
                }
            }
        }
        let con: Vec<_> = muc.iter().filter(|(t, _)| !co.contains_key(t)).collect();
        for (i, phan) in con.chunks(lo.max(1)).enumerate() {
            if dung.load(Ordering::Relaxed) {
                break;
            }
            let nguon = tam.join("nguon");
            let _ = std::fs::remove_dir_all(&nguon);
            std::fs::create_dir_all(&nguon)?;
            let mut ds = String::new();
            for (t, w) in phan {
                let (x, ts) = doc_wav(&std::fs::read(w)?).with_context(|| w.display().to_string())?;
                ghi_wav16(&nguon.join(format!("{t}.wav")), &chuan_hoa(&x, ts, tang_db), TAN_SO)?;
                ds += &format!(
                    "  <Source Path=\"{0}.wav\" Conversion=\"Vorbis Quality High\" Destination=\"{0}.wem\"/>\n",
                    xml_thoat(t)
                );
            }
            let ds_file = tam.join("ds.wsources");
            std::fs::write(
                &ds_file,
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ExternalSourcesList SchemaVersion=\"1\" Root=\"{}\">\n{ds}</ExternalSourcesList>\n",
                    xml_thoat(&nguon.to_string_lossy())
                ),
            )?;
            bao(&format!("Wwise đang mã hóa {}/{} câu...", (i * lo + phan.len()).min(con.len()), con.len()));
            let ra = tam.join("ra");
            let _ = std::fs::remove_dir_all(&ra);
            chay_wwise(wwise, &["convert-external-source".as_ref(), du_an.as_os_str(), "--platform".as_ref(), "Windows".as_ref(), "--source-file".as_ref(), ds_file.as_os_str(), "--output".as_ref(), ra.as_os_str()], &mut nk)?;
            let mut cho = vec![ra.clone()];
            while let Some(d) = cho.pop() {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        cho.push(p);
                    } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wem")) {
                        let dich = thu_ra.join(p.file_name().unwrap());
                        if std::fs::rename(&p, &dich).is_err() {
                            std::fs::copy(&p, &dich)?;
                        }
                        co.insert(p.file_stem().unwrap().to_string_lossy().into_owned(), dich);
                    }
                }
            }
        }
        Ok(co)
    })();
    let _ = std::fs::remove_dir_all(tam);
    kq
}

/// Zip cho DMM: `<ten_mod>/0006/<thư mục game>/<tên>.wem`. `muc` = (tên, thư mục trong game).
/// → số file đã ghi.
pub fn dong_goi_zip(
    zip_ra: &Path,
    ten_mod: &str,
    muc: &[(String, String)],
    wem: &BTreeMap<String, PathBuf>,
) -> Result<usize> {
    let tam = zip_ra.with_extension("zip.tam");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&tam)?);
    let tuy: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    let mut so = 0;
    for (t, thu_muc) in muc {
        if let Some(p) = wem.get(t) {
            z.start_file(format!("{ten_mod}/0006/{thu_muc}/{t}.wem"), tuy)?;
            z.write_all(&std::fs::read(p)?)?;
            so += 1;
        }
    }
    z.finish()?;
    std::fs::rename(&tam, zip_ra)?;
    Ok(so)
}

#[cfg(test)]
mod test {
    use super::*;

    fn wav_thu(ts: u32, n: usize) -> Vec<u8> {
        let tam = std::env::temp_dir().join(format!("vh_wav_{ts}_{n}.wav"));
        let x: Vec<f32> = (0..n).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        ghi_wav16(&tam, &x, ts).unwrap();
        std::fs::read(tam).unwrap()
    }

    #[test]
    fn doc_ghi_wav_khu_hoi() {
        let (x, ts) = doc_wav(&wav_thu(24000, 2400)).unwrap();
        assert_eq!((x.len(), ts), (2400, 24000));
        let y = chuan_hoa(&x, ts, 0.0);
        assert_eq!(y.len(), 4800);
        let m = chuan_hoa(&x, ts, 12.0);
        assert!(m.iter().all(|v| v.abs() <= 1.0));
        assert!(m.iter().cloned().fold(0.0, f32::max) > y.iter().cloned().fold(0.0, f32::max));
    }

    #[test]
    fn wav_hong_bi_tu_choi() {
        assert!(doc_wav(b"khong phai wav").is_err());
    }

    /// Mã hóa thật bằng Wwise: `WAV=<file wav> cargo test --release -- --ignored wwise_that`
    #[test]
    #[ignore]
    fn wwise_that() {
        let w = tim_wwise_console().expect("không thấy WwiseConsole");
        let nguon = PathBuf::from(std::env::var("WAV").expect("WAV=<file>"));
        let ra = std::env::temp_dir().join("vh_wwise_ra");
        let _ = std::fs::remove_dir_all(&ra);
        let muc = vec![("thu_1".to_string(), nguon)];
        let dung = AtomicBool::new(false);
        let co = ma_hoa(&muc, &w, &ra, &std::env::temp_dir().join("vh_wwise_tam"), 6.0, 3000,
            &std::env::temp_dir().join("vh_wwise_nk.txt"), &dung, &|s| eprintln!("{s}")).unwrap();
        let wem = std::fs::read(&co["thu_1"]).unwrap();
        assert_eq!(&wem[0..4], b"RIFF");
        let zip = std::env::temp_dir().join("vh_wwise.zip");
        let n = dong_goi_zip(&zip, "LongTieng_Thu", &[("thu_1".into(), "sound/windows/english(us)".into())], &co).unwrap();
        assert_eq!(n, 1);
    }
}
