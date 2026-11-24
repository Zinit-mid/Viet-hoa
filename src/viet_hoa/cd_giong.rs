//! Lồng tiếng Việt cho Crimson Desert (chuyển từ viet_hoa_cd.py: ke_hoach_long_tieng, tao_mau_nhom,
//! long_tieng_phan). Phần A = nhân vật có tên (unique_*), phần B = dân thường.
//! Quy trình: đọc 0006\0.pamt → ghép file giọng (.wem) ↔ câu phụ đề theo đuôi tên → nhóm theo
//! người nói → giọng mẫu cắt từ chính giọng gốc (vgmstream) → VieNeu → Wwise → zip cho DMM.
//! Dùng chung thư mục làm việc với app cũ (`<ổ>:\DichGame_LongTieng\<A|B>\<giọng>`) nên làm tiếp được.
//! Chưa có: tự sửa giới tính bằng cao độ (F0) và học phiên âm bằng Claude – vẫn dùng phan_bo.json cũ.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Result};
use regex::Regex;

use super::cd_dinh_dang::{self as cd, TepPamt};
use super::crimson_desert::CrimsonDesert;
use super::{giong_vieneu as gv, phien_am, wwise};
use crate::duong_dan;

pub struct CauGiong {
    pub f: TepPamt,
    pub ten: String,
    pub en: Option<String>,
    pub vi: Option<String>,
    pub giay: f64,
}

#[derive(Debug, Default)]
pub struct KetQuaCd {
    pub cau: usize,
    pub giong: usize,
    pub thieu_mau: Vec<String>,
    pub tong_game: usize,
}

/// Tên file giọng → người nói: unique_<tên>, unique_shop_<loại>, nhm_adult_soldier_1...
pub fn nhom_giong(ten: &str) -> String {
    let t: Vec<&str> = ten.split('_').collect();
    if t[0] == "unique" && t.len() > 2 {
        return if t[1] == "shop" { t[..3].join("_") } else { t[..2].join("_") };
    }
    for (i, x) in t.iter().take(6).enumerate() {
        if !x.is_empty() && x.chars().all(|c| c.is_ascii_digit()) {
            return t[..=i].join("_");
        }
    }
    t[..t.len().min(3)].join("_")
}

/// Bỏ thẻ và ký hiệu để máy đọc.
pub fn lam_sach_loi(s: &str) -> String {
    static R: OnceLock<[Regex; 4]> = OnceLock::new();
    let r = R.get_or_init(|| {
        [
            Regex::new(r"<[^>]*>|\{[^}]*\}|\[[^\]]*\]").unwrap(),
            Regex::new(r"\s+").unwrap(),
            Regex::new(r"[,;:]\s*([?!.])").unwrap(),
            Regex::new(r"\s+([,.?!;:…])").unwrap(),
        ]
    });
    let s = r[0].replace_all(s, " ");
    let s = s.replace("\\n", " ").replace('\n', " ").replace('*', "");
    let s = r[1].replace_all(&s, " ");
    let s = r[2].replace_all(s.trim(), "$1");
    let s = r[3].replace_all(&s, "$1");
    s.trim_matches(|c| c == ' ' || c == ',' || c == ';' || c == ':').to_string()
}

/// Thời lượng (giây) từ phần đầu file .wem (RIFF Wwise).
pub fn giay_wem(dau: &[u8], co_goc: u32) -> f64 {
    if dau.len() < 12 || (&dau[..4] != b"RIFF" && &dau[..4] != b"RIFX") {
        return 0.0;
    }
    let u32at = |o: usize| u32::from_le_bytes(dau[o..o + 4].try_into().unwrap());
    let (mut o, mut tan_so, mut bps, mut mau) = (12usize, 0u32, 0u32, 0u32);
    while o + 8 <= dau.len() {
        let co = u32at(o + 4) as usize;
        if &dau[o..o + 4] == b"fmt " && o + 8 + 12 <= dau.len() {
            let ma = u16::from_le_bytes([dau[o + 8], dau[o + 9]]);
            tan_so = u32at(o + 12);
            bps = u32at(o + 16);
            if ma == 0xFFFF && co >= 0x1C && o + 8 + 0x18 + 4 <= dau.len() {
                mau = u32at(o + 8 + 0x18);
            }
        }
        o += 8 + co + (co & 1);
    }
    if mau > 0 && tan_so > 0 {
        mau as f64 / tan_so as f64
    } else if bps > 0 {
        co_goc as f64 / bps as f64
    } else {
        0.0
    }
}

fn doc_dau(thu_muc_nhom: &Path, f: &TepPamt) -> Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let mut fh = std::fs::File::open(thu_muc_nhom.join(format!("{}.paz", f.paz)))?;
    fh.seek(SeekFrom::Start(f.vt as u64))?;
    let mut b = vec![0u8; (f.nen as usize).min(512)];
    fh.read_exact(&mut b)?;
    Ok(b)
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt()
}

pub fn vgmstream() -> Result<PathBuf> {
    let p = duong_dan::cong_cu().join("vgmstream").join("vgmstream-cli.exe");
    if p.is_file() {
        Ok(p)
    } else {
        Err(anyhow!("thiếu {} – chạy scripts\\tai_cong_cu.ps1 để tải vgmstream", p.display()))
    }
}

/// Giải mã .wem → WAV bằng vgmstream.
fn giai_ma_wem(vgm: &Path, wem: &Path, ra: &Path) -> bool {
    let _ = std::fs::remove_file(ra);
    let mut l = Command::new(vgm);
    l.arg("-o").arg(ra).arg(wem).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        l.creation_flags(0x0800_0000);
    }
    l.status().is_ok_and(|s| s.success()) && ra.is_file()
}

/// Ổ đĩa còn trống bao nhiêu byte.
fn con_trong(thu_muc: &Path) -> Option<u64> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut tu = thu_muc;
    while !tu.exists() {
        tu = tu.parent()?;
    }
    let mut trong = 0u64;
    unsafe {
        GetDiskFreeSpaceExW(&HSTRING::from(tu.as_os_str()), Some(&mut trong), None, None).ok()?;
    }
    Some(trong)
}


/// Cao độ giọng (Hz, trung vị) của mảng mono; None nếu không đo được. Khung 40 ms, tự tương quan.
pub fn cao_do(x: &[f32], sr: u32) -> Option<f64> {
    let fr = (0.04 * sr as f64) as usize;
    let (lo, hi) = ((sr / 450) as usize, (sr / 60) as usize);
    if fr < hi + 1 || x.len() <= fr {
        return None;
    }
    let mut r: Vec<f64> = Vec::new();
    let mut i = 0;
    while i + fr < x.len() {
        let y = &x[i..i + fr];
        i += fr;
        if rms(y) < 0.02 {
            continue;
        }
        let tb = y.iter().sum::<f32>() / fr as f32;
        let y: Vec<f32> = y.iter().map(|v| v - tb).collect();
        let c0: f64 = y.iter().map(|v| (*v as f64).powi(2)).sum();
        let (mut tot, mut k) = (f64::MIN, lo);
        for lag in lo..hi {
            let c: f64 = y[..fr - lag].iter().zip(&y[lag..]).map(|(a, b)| *a as f64 * *b as f64).sum();
            if c > tot {
                (tot, k) = (c, lag);
            }
        }
        if tot > 0.3 * c0 {
            r.push(sr as f64 / k as f64);
        }
    }
    if r.len() < 5 {
        return None;
    }
    r.sort_by(f64::total_cmp);
    let n = r.len();
    Some(if n % 2 == 1 { r[n / 2] } else { (r[n / 2 - 1] + r[n / 2]) / 2.0 })
}

/// Giọng riêng KHÔNG dùng để xoay vòng (giọng đặc biệt: phù thủy, người già, trẻ em, nhân vật chính).
const KHONG_XOAY: [&str; 7] = ["phu_thuy", "gia", "be_", "nam_chinh.wav", "giong_nam_chinh", "giong_nu_chinh", "nam_am"];

fn la_nu(f: Option<f64>) -> bool {
    f.is_some_and(|f| f >= 175.0)
}
fn la_nam(f: Option<f64>) -> bool {
    f.is_some_and(|f| f <= 160.0)
}

impl CrimsonDesert {
    /// {người nói: [câu]} của phần 'A' (nhân vật có tên) hoặc 'B' (dân thường).
    pub fn ke_hoach_long_tieng(
        &self,
        phan: char,
        bao: &dyn Fn(&str),
    ) -> Result<BTreeMap<String, Vec<CauGiong>>> {
        let nhom_am = self.thu_muc_game.join("0006");
        if !nhom_am.join("0.pamt").is_file() {
            bail!("không thấy thư mục âm thanh tiếng Anh (0006) trong thư mục game");
        }
        bao("Đang đọc danh sách giọng nói trong game...");
        let ds: Vec<TepPamt> = cd::doc_pamt(&nhom_am.join("0.pamt"))?
            .into_iter()
            .filter(|f| f.duoi == ".wem" && f.thu_muc.contains("english"))
            .collect();
        let mut khoa: std::collections::HashMap<String, &str> = Default::default();
        for p in self.nhom.values() {
            for (_, k, v) in &p.ds {
                khoa.entry(k.to_lowercase()).or_insert(v.as_str());
            }
        }
        let mut kq: BTreeMap<String, Vec<CauGiong>> = BTreeMap::new();
        let co_chu = Regex::new("[a-z]").unwrap();
        for (i, f) in ds.iter().enumerate() {
            let giong = nhom_giong(&f.ten);
            if (phan == 'A') != giong.starts_with("unique_") {
                continue;
            }
            let ten = f.ten.clone();
            let t: Vec<String> = ten.to_lowercase().split('_').map(String::from).collect();
            let en = (1..t.len()).find_map(|j| {
                let duoi = t[j..].join("_");
                (co_chu.is_match(&duoi)).then(|| khoa.get(&duoi).map(|v| v.to_string())).flatten()
            });
            let vi = en.as_ref().and_then(|e| self.viec.tim(e)).filter(|v| Some(v) != en.as_ref());
            let giay = doc_dau(&nhom_am, f).map(|d| giay_wem(&d, f.goc)).unwrap_or(0.0);
            kq.entry(giong).or_default().push(CauGiong { f: f.clone(), ten, en, vi, giay });
            if i % 3000 == 0 {
                bao(&format!("Đang đọc danh sách giọng nói... {i}/{}", ds.len()));
            }
        }
        Ok(kq)
    }

    /// Giọng mẫu ~9–15 giây ghép từ chính các câu gốc của người nói này. Có sẵn mau.wav thì dùng luôn.
    fn tao_mau_nhom(&self, cau: &[CauGiong], thu_ra: &Path, vgm: &Path) -> Result<Option<PathBuf>> {
        let mau = thu_ra.join("mau.wav");
        if mau.is_file() {
            return Ok(Some(mau));
        }
        let tam = thu_ra.join("mau_tam");
        std::fs::create_dir_all(&tam)?;
        let mut uu: Vec<&CauGiong> = cau.iter().filter(|c| (3.0..=9.0).contains(&c.giay)).collect();
        uu.sort_by(|a, b| b.giay.total_cmp(&a.giay));
        let mut phu: Vec<&CauGiong> = cau
            .iter()
            .filter(|c| (1.2..3.0).contains(&c.giay) || (c.giay > 9.0 && c.giay <= 14.0))
            .collect();
        phu.sort_by(|a, b| b.giay.total_cmp(&a.giay));
        uu.extend(phu);
        let nhom_am = self.thu_muc_game.join("0006");
        let (mut phan, mut tong) = (Vec::<Vec<f32>>::new(), 0.0f64);
        for c in uu.into_iter().take(16) {
            let (wem, ra) = (tam.join("x.wem"), tam.join("x.wav"));
            let Ok(b) = cd::doc_tep(&nhom_am, &c.f) else { continue };
            if std::fs::write(&wem, b).is_err() || !giai_ma_wem(vgm, &wem, &ra) {
                continue;
            }
            let Ok(raw) = std::fs::read(&ra) else { continue };
            let Ok((x, ts)) = wwise::doc_wav(&raw) else { continue };
            let x = wwise::chuan_hoa(&x, ts, 0.0);
            if x.len() < 48000 || rms(&x) < 0.01 {
                continue;
            }
            tong += x.len() as f64 / 48000.0;
            phan.push(x);
            if tong >= 9.0 {
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&tam);
        if phan.is_empty() {
            return Ok(None);
        }
        let lang = vec![0f32; 14400];
        let mut y: Vec<f32> = phan.iter().flat_map(|p| p.iter().chain(lang.iter()).copied()).collect();
        y.truncate(15 * 48000);
        wwise::ghi_wav16(&mau, &y, 48000)?;
        Ok(Some(mau))
    }

    /// Giọng người dùng tự ghi âm: `<lv>\giong_rieng\phan_bo.json` {"A": {giọng: file.wav}, "B": {...}}.
    /// Gán/đổi file → chép làm mau.wav, xóa câu đã đọc + .wem cũ để làm lại.
    fn dung_giong_rieng(thu_muc_lv: &Path, phan: char, g: &str, thu: &Path, cau: &[CauGiong]) {
        let goc = thu_muc_lv.join("giong_rieng");
        let ghi = thu.join("_giong_rieng.txt");
        let ten = std::fs::read(goc.join("phan_bo.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .and_then(|j| j.get(phan.to_string())?.get(g)?.as_str().map(String::from));
        let xoa_cu = || {
            let _ = std::fs::remove_dir_all(thu.join("wav"));
            let Ok(rd) = std::fs::read_dir(thu_muc_lv.join(phan.to_string())) else { return };
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with("_wem_") && n.ends_with("db") {
                    for c in cau {
                        let _ = std::fs::remove_file(e.path().join(format!("{}.wem", c.ten)));
                    }
                }
            }
        };
        let ten = ten.map(|t| if t.to_lowercase().ends_with(".wav") { t } else { format!("{t}.wav") });
        let Some(ten) = ten.filter(|t| goc.join(t).is_file()) else {
            if ghi.is_file() {
                // người dùng bỏ gán giọng riêng → xóa mẫu + câu cũ để nhái lại giọng gốc
                xoa_cu();
                let _ = std::fs::remove_file(thu.join("mau.wav"));
                let _ = std::fs::remove_file(&ghi);
            }
            return;
        };
        let nguon = goc.join(&ten);
        let Ok(b) = std::fs::read(&nguon) else { return };
        let dau = format!("{:x} {ten}", md5::compute(&b));
        if std::fs::read_to_string(&ghi).is_ok_and(|s| s.trim() == dau) {
            return;
        }
        xoa_cu();
        if std::fs::write(thu.join("mau.wav"), &b).is_ok() {
            let _ = std::fs::write(&ghi, dau);
        }
    }

    /// Đo cao độ giọng gốc từng nhân vật → nhân vật NỮ đang bị gán giọng nam (và ngược lại) thì đổi sang
    /// giọng cùng giới trong kho giọng riêng. Ghi lại phan_bo.json (sao lưu .truoc_sua_gioi_tinh).
    /// Người đã gán tay (`_tay`) không bị đổi. → [(người nói, giọng cũ, giọng mới)]
    pub fn sua_gioi_tinh(
        &self,
        phan: char,
        nhom: &BTreeMap<String, Vec<CauGiong>>,
        thu_muc_lv: &Path,
        vgm: &Path,
        dung: &AtomicBool,
        bao: &dyn Fn(&str),
    ) -> Result<Vec<(String, String, String)>> {
        use serde_json::{json, Value};
        let goc = thu_muc_lv.join("giong_rieng");
        let tep = goc.join("phan_bo.json");
        let Ok(b) = std::fs::read(&tep) else { return Ok(vec![]) };
        let mut pb: Value = serde_json::from_slice(&b)?;
        let ph = phan.to_string();
        let Some(mut gan) = pb.get(&ph).and_then(Value::as_object).cloned().filter(|o| !o.is_empty()) else {
            return Ok(vec![]);
        };
        // cao độ các giọng riêng
        let mut cd_giong: BTreeMap<String, Option<f64>> = BTreeMap::new();
        for ten in gan.values().filter_map(Value::as_str) {
            let f0 = std::fs::read(goc.join(ten)).ok().and_then(|b| wwise::doc_wav(&b).ok()).and_then(|(x, sr)| cao_do(&x, sr));
            cd_giong.insert(ten.to_string(), f0);
        }
        let xoay = |dk: fn(Option<f64>) -> bool| -> Vec<String> {
            cd_giong.iter().filter(|(t, f)| dk(**f) && !KHONG_XOAY.iter().any(|k| t.contains(k))).map(|(t, _)| t.clone()).collect()
        };
        let (nu, nam) = (xoay(la_nu), xoay(la_nam));
        // cao độ giọng gốc từng nhân vật (lưu đệm)
        let dem_tep = thu_muc_lv.join(&ph).join("_cao_do_goc.json");
        let mut dem: BTreeMap<String, Option<f64>> =
            std::fs::read(&dem_tep).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let tam = thu_muc_lv.join(&ph).join("_do_cao_do");
        let nhom_am = self.thu_muc_game.join("0006");
        let ten_gan: Vec<String> = {
            let mut v: Vec<String> = gan.keys().cloned().collect();
            v.sort();
            v
        };
        for (i, g) in ten_gan.iter().enumerate() {
            if dem.contains_key(g) || !nhom.contains_key(g) {
                continue;
            }
            bao(&format!("Đang kiểm tra giới tính giọng gốc {}/{}: {g}", i + 1, ten_gan.len()));
            std::fs::create_dir_all(&tam)?;
            let mut cau: Vec<&CauGiong> = nhom[g].iter().filter(|c| (1.5..=9.0).contains(&c.giay)).collect();
            cau.sort_by(|a, b| b.giay.total_cmp(&a.giay));
            let mut phan_am: Vec<f32> = Vec::new();
            for c in cau.into_iter().take(4) {
                let (wem, ra) = (tam.join("x.wem"), tam.join("x.wav"));
                let Ok(d) = cd::doc_tep(&nhom_am, &c.f) else { continue };
                if std::fs::write(&wem, d).is_err() || !giai_ma_wem(vgm, &wem, &ra) {
                    continue;
                }
                if let Some((x, ts)) = std::fs::read(&ra).ok().and_then(|b| wwise::doc_wav(&b).ok()) {
                    phan_am.extend(wwise::chuan_hoa(&x, ts, 0.0));
                }
            }
            dem.insert(g.clone(), if phan_am.is_empty() { None } else { cao_do(&phan_am, 48000) });
            if dung.load(Ordering::Relaxed) {
                break;
            }
        }
        let _ = std::fs::remove_dir_all(&tam);
        std::fs::create_dir_all(dem_tep.parent().unwrap())?;
        std::fs::write(&dem_tep, serde_json::to_vec_pretty(&dem)?)?;

        let tay: Vec<String> = pb
            .get("_tay")
            .and_then(|t| t.get(&ph))
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let (mut doi, mut d_nu, mut d_nam) = (Vec::new(), 0usize, 0usize);
        let mut thu_tu: Vec<&String> = ten_gan.iter().collect();
        thu_tu.sort_by_key(|g| std::cmp::Reverse(nhom.get(*g).map_or(0, Vec::len)));
        for g in thu_tu {
            if tay.contains(g) {
                continue;
            }
            let f0 = dem.get(g).copied().flatten();
            let cu = gan[g].as_str().unwrap_or("").to_string();
            let f_cu = cd_giong.get(&cu).copied().flatten();
            let moi = if la_nu(f0) && !la_nu(f_cu) && !nu.is_empty() {
                d_nu += 1;
                Some(nu[(d_nu - 1) % nu.len()].clone())
            } else if la_nam(f0) && !la_nam(f_cu) && !nam.is_empty() {
                d_nam += 1;
                Some(nam[(d_nam - 1) % nam.len()].clone())
            } else {
                None
            };
            if let Some(m) = moi.filter(|m| *m != cu) {
                gan.insert(g.clone(), json!(m));
                doi.push((g.clone(), cu, m));
            }
        }
        if !doi.is_empty() {
            let sao = PathBuf::from(format!("{}.truoc_sua_gioi_tinh", tep.display()));
            if !sao.exists() {
                std::fs::copy(&tep, &sao)?;
            }
            pb[&ph] = Value::Object(gan);
            let t = PathBuf::from(format!("{}.tam", tep.display()));
            std::fs::write(&t, serde_json::to_vec_pretty(&pb)?)?;
            std::fs::rename(&t, &tep)?;
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(thu_muc_lv.join(format!("doi_gioi_tinh_{phan}.txt")))?;
            for (g, cu, m) in &doi {
                let _ = writeln!(f, "{g}: {cu} -> {m} (giọng gốc {:.0} Hz)", dem.get(g).copied().flatten().unwrap_or(0.0));
            }
        }
        Ok(doi)
    }

    /// Cả quy trình cho phần A/B; ghi zip DMM vào `zip_ra`. Chạy lại = làm tiếp. → None nếu bị dừng.
    pub fn long_tieng_phan(
        &self,
        phan: char,
        thu_muc_lv: &Path,
        zip_ra: &Path,
        tang_db: f64,
        dung: &AtomicBool,
        bao: &dyn Fn(&str),
        tien_do: &dyn Fn(usize, usize, &str),
    ) -> Result<Option<KetQuaCd>> {
        let wwise_exe = wwise::tim_wwise_console()
            .ok_or_else(|| anyhow!("không tìm thấy Wwise (WwiseConsole.exe) – cài Wwise bằng Audiokinetic Launcher"))?;
        if gv::tim_vieneu().is_none() {
            bail!("không tìm thấy app VieNeu (bản tăng tốc GPU) – mở VieNeu, cài 'GPU' rồi thử lại");
        }
        let nhom = self.ke_hoach_long_tieng(phan, bao)?;
        let can: Vec<(&String, Vec<&CauGiong>)> = {
            let mut v: Vec<_> = nhom
                .iter()
                .map(|(g, ds)| (g, ds.iter().filter(|c| c.vi.as_deref().is_some_and(|v| !lam_sach_loi(v).is_empty())).collect::<Vec<_>>()))
                .filter(|(_, ds)| !ds.is_empty())
                .collect();
            v.sort_by_key(|(_, d)| std::cmp::Reverse(d.len()));
            v
        };
        let tong: usize = can.iter().map(|(_, d)| d.len()).sum();
        if tong == 0 {
            bail!("chưa có câu thoại nào của phần này được Việt hóa – dịch chữ (các đợt) trước");
        }
        std::fs::create_dir_all(thu_muc_lv)?;
        let can_gb = tong as f64 * 3.5 * 96000.0 / 1e9;
        if let Some(t) = con_trong(thu_muc_lv) {
            let t = t as f64 / 1e9;
            if t < can_gb * 1.2 + 2.0 {
                bail!("ổ đĩa chỉ còn {t:.0} GB, cần khoảng {:.0} GB trống", can_gb * 1.2 + 2.0);
            }
        }
        let vgm = vgmstream()?;
        let tam = thu_muc_lv.join("tam");
        // không đo được thì giữ phân bổ cũ
        let _ = self.sua_gioi_tinh(phan, &nhom, thu_muc_lv, &vgm, dung, bao);
        let mut viec: Vec<gv::Nhom> = Vec::new();
        let mut thieu_mau = Vec::new();
        for (i, (g, ds)) in can.iter().enumerate() {
            bao(&format!("Bước 2/4: tạo giọng mẫu {}/{}: {g}", i + 1, can.len()));
            let thu = thu_muc_lv.join(phan.to_string()).join(g.as_str());
            std::fs::create_dir_all(&thu)?;
            Self::dung_giong_rieng(thu_muc_lv, phan, g, &thu, &nhom[*g]);
            let Some(mau) = self.tao_mau_nhom(&nhom[*g], &thu, &vgm)? else {
                thieu_mau.push(g.to_string());
                continue;
            };
            let r = std::fs::read(&mau)
                .ok()
                .and_then(|b| wwise::doc_wav(&b).ok())
                .map(|(x, _)| rms(&x))
                .filter(|r| *r > 0.0)
                .unwrap_or(0.08);
            viec.push(gv::Nhom {
                ten: g.to_string(),
                mau,
                ra: thu.join("wav"),
                rms: r.clamp(0.03, 0.2),
                ds: ds
                    .iter()
                    .map(|c| {
                        let chu = phien_am::doc_duoc(&lam_sach_loi(c.vi.as_deref().unwrap_or("")));
                        (c.ten.clone(), chu, (c.giay * 1000.0).round() / 1000.0)
                    })
                    .collect(),
            });
            if dung.load(Ordering::Relaxed) {
                return Ok(None);
            }
        }
        if !gv::chay(&viec, &tam, 1.4, dung, &|a, b, ct| tien_do(a, b, &format!("Bước 3/4: VieNeu đã đọc{ct}")))? {
            return Ok(None);
        }

        bao("Bước 4/4: Wwise đang mã hóa...");
        let mut muc: Vec<(String, PathBuf)> = Vec::new();
        for v in &viec {
            for d in &v.ds {
                let w = v.ra.join(format!("{}.wav", d.0));
                if w.is_file() {
                    muc.push((d.0.clone(), w));
                }
            }
        }
        // âm lượng thay đổi thì mã hóa lại từ đầu (thư mục theo số dB)
        let thu_wem = thu_muc_lv.join(phan.to_string()).join(format!("_wem_{}db", tang_db as i64));
        let co = wwise::ma_hoa(
            &muc,
            &wwise_exe,
            &thu_wem,
            &tam.join("wwise"),
            tang_db,
            3000,
            &thu_muc_lv.join(format!("wwise_nhat_ky_{phan}.txt")),
            dung,
            bao,
        )?;
        if dung.load(Ordering::Relaxed) {
            return Ok(None);
        }
        let duong: Vec<(String, String)> = nhom
            .values()
            .flatten()
            .map(|c| (c.ten.clone(), c.f.thu_muc.clone()))
            .collect();
        let ds_zip: Vec<(String, String)> = {
            let m: std::collections::HashMap<&str, &str> = duong.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
            muc.iter().filter_map(|(t, _)| m.get(t.as_str()).map(|d| (t.clone(), d.to_string()))).collect()
        };
        bao("Đang đóng gói cho DMM...");
        let ten_mod = if phan == 'A' { "LongTieng_NhanVat" } else { "LongTieng_DanThuong" };
        let so = wwise::dong_goi_zip(zip_ra, ten_mod, &ds_zip, &co)?;
        Ok(Some(KetQuaCd {
            cau: so,
            giong: viec.len(),
            thieu_mau,
            tong_game: nhom.values().map(Vec::len).sum(),
        }))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn nhom_giong_dung() {
        assert_eq!(nhom_giong("unique_kliff_0001_abc"), "unique_kliff");
        assert_eq!(nhom_giong("unique_shop_smith_12"), "unique_shop_smith");
        assert_eq!(nhom_giong("nhm_adult_soldier_1_xyz"), "nhm_adult_soldier_1");
        assert_eq!(nhom_giong("abc"), "abc");
    }

    #[test]
    fn lam_sach_dung() {
        assert_eq!(lam_sach_loi("Xin chào <br/> {Param1} , ngươi *cười*  !"), "Xin chào, ngươi cười!");
    }

    /// Giọng mẫu thật của 1 người nói (ghi vào thư mục tạm): CD=<thư mục game> NGUOI=unique_kliff
    #[test]
    #[ignore]
    fn mau_that() {
        let tm = PathBuf::from(std::env::var("CD").unwrap());
        let g = std::env::var("NGUOI").unwrap_or("unique_kliff".into());
        let cd = CrimsonDesert::nap(&tm).unwrap();
        let k = cd.ke_hoach_long_tieng('A', &|_| {}).unwrap();
        let ra = std::env::temp_dir().join("vh_cd_mau");
        let _ = std::fs::remove_dir_all(&ra);
        std::fs::create_dir_all(&ra).unwrap();
        let mau = cd.tao_mau_nhom(&k[&g], &ra, &vgmstream().unwrap()).unwrap().unwrap();
        let b = std::fs::read(&mau).unwrap();
        let (x, ts) = wwise::doc_wav(&b).unwrap();
        println!("{g}: {} câu, mẫu {:.1}s {ts}Hz rms {:.3}", k[&g].len(), x.len() as f64 / ts as f64, rms(&x));
        assert!(x.len() as f64 / ts as f64 > 5.0);
    }

    /// Kế hoạch thật (chỉ đọc): CD=<thư mục game> cargo test --release -- --ignored ke_hoach_that --nocapture
    #[test]
    #[ignore]
    fn ke_hoach_that() {
        let tm = PathBuf::from(std::env::var("CD").unwrap());
        let cd = CrimsonDesert::nap(&tm).unwrap();
        let t0 = std::time::Instant::now();
        for phan in ['A', 'B'] {
            let k = cd.ke_hoach_long_tieng(phan, &|_| {}).unwrap();
            let cau: usize = k.values().map(Vec::len).sum();
            let viet: usize = k.values().flatten().filter(|c| c.vi.is_some()).count();
            let co_en: usize = k.values().flatten().filter(|c| c.en.is_some()).count();
            let giay: f64 = k.values().flatten().map(|c| c.giay).sum();
            println!("phần {phan}: {} giọng, {cau} câu, {co_en} có phụ đề, {viet} đã Việt hóa, {giay:.0} s ({:?})", k.len(), t0.elapsed());
            for (g, ds) in k.iter().take(5) {
                println!("  {g}: {} câu", ds.len());
            }
        }
    }

    /// So với app cũ: dùng bản phan_bo TRƯỚC khi sửa + cao độ đã đệm → phải ra gần đúng doi_gioi_tinh_A.txt.
    /// CD=<thư mục game> cargo test --release -- --ignored gioi_tinh_that --nocapture
    #[test]
    #[ignore]
    fn gioi_tinh_that() {
        let tm = PathBuf::from(std::env::var("CD").unwrap());
        let cd = CrimsonDesert::nap(&tm).unwrap();
        let cu = PathBuf::from(r"G:\DichGame_LongTieng");
        let lv = std::env::temp_dir().join("vh_cd_gt");
        let _ = std::fs::remove_dir_all(&lv);
        std::fs::create_dir_all(lv.join("giong_rieng")).unwrap();
        std::fs::create_dir_all(lv.join("A")).unwrap();
        for e in std::fs::read_dir(cu.join("giong_rieng")).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.ends_with(".wav") {
                std::fs::copy(e.path(), lv.join("giong_rieng").join(&n)).unwrap();
            }
        }
        std::fs::copy(cu.join("giong_rieng/phan_bo.json.truoc_sua_gioi_tinh"), lv.join("giong_rieng/phan_bo.json")).unwrap();
        std::fs::copy(cu.join("A/_cao_do_goc.json"), lv.join("A/_cao_do_goc.json")).unwrap();
        let k = cd.ke_hoach_long_tieng('A', &|_| {}).unwrap();
        let doi = cd.sua_gioi_tinh('A', &k, &lv, &vgmstream().unwrap(), &AtomicBool::new(false), &|_| {}).unwrap();
        let mong: std::collections::BTreeSet<String> = std::fs::read_to_string(cu.join("doi_gioi_tinh_A.txt"))
            .unwrap()
            .lines()
            .filter_map(|l| l.split(':').next().map(String::from))
            .collect();
        let co: std::collections::BTreeSet<String> = doi.iter().map(|d| d.0.clone()).collect();
        println!("app cũ đổi {} người, bản Rust đổi {}; giống nhau {}", mong.len(), co.len(), mong.intersection(&co).count());
        for d in doi.iter().take(5) {
            println!("  {} : {} -> {}", d.0, d.1, d.2);
        }
        assert!(mong.intersection(&co).count() as f64 >= mong.len() as f64 * 0.9);
    }
}
