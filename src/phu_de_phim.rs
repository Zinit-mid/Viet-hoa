//! Phụ đề phim tiếng Việt: link YouTube / trang video hoặc file phim trên máy → tải về (yt-dlp) →
//! AI nghe ra chữ (faster-whisper large-v3-turbo, card màn hình) → dịch (Claude / offline) →
//! file .srt cùng tên đặt cạnh phim → mở phim bằng VLC kèm phụ đề.
//! Chuyển từ phu_de_phim.py. Dùng Python riêng của app (cong_cu\python đã có faster-whisper, yt-dlp, torch CUDA);
//! mô hình Whisper ở DichGame_AI\phu_de_phim\_mo_hinh. Dữ liệu từng phim ở `<thư mục phim>\<md5 nguồn>`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use crate::duong_dan;
use crate::viet_hoa::viec_dich::{BangTen, ViecDich};

const HUONG_DAN: &str = "Bạn là dịch giả phụ đề phim chuyên nghiệp. Bạn nhận một mảng JSON các dòng phụ đề \
(theo đúng thứ tự trong phim, có thể là tiếng Anh hoặc ngôn ngữ khác) và trả về mảng JSON bản dịch tiếng Việt.

Quy tắc bắt buộc:
- CHỈ trả về một mảng JSON hợp lệ, CÙNG SỐ PHẦN TỬ, CÙNG THỨ TỰ. Không giải thích, không ```.
- Mỗi phần tử dịch đúng dòng tương ứng; dựa vào các dòng trước / sau để hiểu ngữ cảnh, xưng hô cho tự nhiên.
- Văn phong phụ đề Việt: ngắn gọn, tự nhiên, dễ đọc nhanh; xưng hô hợp quan hệ nhân vật.
- Giữ nguyên tên người, địa danh, thương hiệu. Dòng có thể bị nghe sai chính tả – đoán nghĩa hợp lý theo ngữ cảnh.
- Dòng chỉ là tiếng động / nhạc / vô nghĩa thì dịch ngắn gọn hoặc trả về nguyên văn.
";

/// Thư mục dữ liệu phim (chung với app cũ nếu đã có DichGame_AI\phu_de_phim).
pub fn thu_muc() -> PathBuf {
    let p = duong_dan::goc().join("DichGame_AI").join("phu_de_phim");
    let _ = std::fs::create_dir_all(&p);
    p
}

pub fn la_link(s: &str) -> bool {
    let s = s.trim_start().to_lowercase();
    s.starts_with("http://") || s.starts_with("https://")
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Phim {
    pub duong_dan: PathBuf,
    #[serde(default)]
    pub ten: String,
}

#[derive(Deserialize)]
struct Tu(f64, f64, String);

#[derive(Deserialize)]
struct Doan {
    a: f64,
    b: f64,
    #[serde(default)]
    chu: String,
    #[serde(default)]
    tu: Vec<Tu>,
}

#[derive(Deserialize)]
struct ChuGoc {
    #[serde(default)]
    ngon_ngu: Option<String>,
    doan: Vec<Doan>,
}

pub type Dong = (f64, f64, String);

/// Đoạn Whisper (có mốc từng từ) → các dòng phụ đề ngắn.
fn chia_dong(doan: &[Doan]) -> Vec<Dong> {
    let (toi_da_giay, toi_da_chu) = (6.0, 84usize);
    let mut ra: Vec<Dong> = Vec::new();
    for d in doan {
        if d.tu.is_empty() {
            if !d.chu.trim().is_empty() {
                ra.push((d.a, d.b, d.chu.trim().to_string()));
            }
            continue;
        }
        let mut cur: Vec<&Tu> = Vec::new();
        for (i, w) in d.tu.iter().enumerate() {
            cur.push(w);
            let chu: String = cur.iter().map(|x| x.2.as_str()).collect::<String>().trim().to_string();
            let n = chu.chars().count();
            let dai = cur.last().unwrap().1 - cur[0].0;
            let t = w.2.trim();
            let het_cau = t.ends_with(['.', '?', '!', '。', '？', '！']) && n >= 15;
            let phay = t.ends_with([',', ';', ':', '，'])
                && (dai >= toi_da_giay * 0.6 || n as f64 >= toi_da_chu as f64 * 0.6);
            if het_cau || phay || dai >= toi_da_giay || n >= toi_da_chu || i == d.tu.len() - 1 {
                if !chu.is_empty() {
                    ra.push((cur[0].0, cur.last().unwrap().1, chu));
                }
                cur.clear();
            }
        }
    }
    // nối dòng quá ngắn với dòng sau (< 0,6 giây và sát nhau)
    let mut gon: Vec<Dong> = Vec::new();
    for (a, b, c) in ra {
        if let Some(l) = gon.last_mut() {
            if l.1 - l.0 < 0.6 && a - l.1 < 0.3 && l.2.chars().count() + c.chars().count() < 84 {
                l.1 = b;
                l.2 = format!("{} {c}", l.2);
                continue;
            }
        }
        gon.push((a, b, c));
    }
    gon
}

fn gio(s: f64) -> String {
    let ms = (s.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

/// Ngắt dòng dài làm 2 dòng ở khoảng trắng gần giữa.
fn xuong_dong(chu: &str) -> String {
    let n = chu.chars().count();
    if n <= 42 {
        return chu.to_string();
    }
    let giua = n / 2;
    let vt: Vec<usize> = chu.chars().enumerate().filter(|(_, c)| *c == ' ').map(|(i, _)| i).collect();
    let Some(&i) = vt.iter().min_by_key(|&&x| (x as i64 - giua as i64).abs()) else {
        return chu.to_string();
    };
    let (a, b): (String, String) = (chu.chars().take(i).collect(), chu.chars().skip(i).collect());
    format!("{}\n{}", a.trim(), b.trim())
}

/// Nội dung .srt (UTF-8 có BOM để trình phát Windows đọc đúng dấu).
pub fn noi_dung_srt(dong: &[Dong]) -> Vec<u8> {
    let mut s = String::new();
    for (i, (a, b, c)) in dong.iter().enumerate() {
        let nxt = dong.get(i + 1).map(|d| d.0 - 0.05).unwrap_or(f64::INFINITY);
        // hiện lâu thêm chút, không đè lên dòng sau
        let b2 = (b + 0.4).min(nxt).max((a + 0.8).min(nxt)).max(a + 0.2);
        s += &format!("{}\n{} --> {}\n{}\n\n", i + 1, gio(*a), gio(b2), xuong_dong(c));
    }
    let mut v = vec![0xEF, 0xBB, 0xBF];
    v.extend(s.trim_end_matches('\n').as_bytes());
    v.extend(b"\n");
    v
}

/// Mở phim; có VLC thì nạp thẳng file phụ đề (chắc chắn hiện), không thì mở bằng trình mặc định.
pub fn mo_phim(phim: &Path, srt: Option<&Path>) {
    let mut ung: Vec<PathBuf> = vec![duong_dan::cong_cu().join("vlc").join("vlc.exe")];
    for k in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(g) = std::env::var_os(k) {
            ung.push(PathBuf::from(g).join("VideoLAN").join("VLC").join("vlc.exe"));
        }
    }
    if let Some(vlc) = ung.into_iter().find(|p| p.is_file()) {
        let mut c = Command::new(vlc);
        c.arg(phim);
        if let Some(s) = srt.filter(|s| s.is_file()) {
            c.arg(format!("--sub-file={}", s.display()));
        }
        if c.spawn().is_ok() {
            return;
        }
    }
    let _ = Command::new("cmd").args(["/c", "start", ""]).arg(phim).spawn();
}

pub struct ViecPhim {
    pub nguon: String,
    tm: PathBuf,
    pub phim: Option<Phim>,
    pub dong: Vec<Dong>,
    pub ngon_ngu: Option<String>,
    pub viec: ViecDich,
}

impl ViecPhim {
    pub fn moi(nguon: &str) -> Result<Self> {
        let nguon = nguon.trim().trim_matches('"').to_string();
        let ma = format!("{:x}", md5::compute(nguon.as_bytes()));
        let tm = thu_muc().join(&ma[..12]);
        std::fs::create_dir_all(&tm)?;
        let mut phim: Option<Phim> = std::fs::read(tm.join("phim.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|p: &Phim| p.duong_dan.is_file());
        if !la_link(&nguon) && Path::new(&nguon).is_file() {
            phim = Some(Phim {
                duong_dan: PathBuf::from(&nguon),
                ten: Path::new(&nguon).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            });
        }
        let mut v = Self {
            nguon,
            viec: ViecDich::moi(tm.clone(), HUONG_DAN, BangTen::tao(std::iter::empty::<&str>())),
            tm,
            phim,
            dong: Vec::new(),
            ngon_ngu: None,
        };
        v.nap_chu();
        Ok(v)
    }

    fn nap_chu(&mut self) {
        let Some(d) = std::fs::read(self.tm.join("chu_goc.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<ChuGoc>(&b).ok())
        else {
            self.dong.clear();
            return;
        };
        self.dong = chia_dong(&d.doan);
        self.ngon_ngu = d.ngon_ngu;
    }

    /// Dòng chưa có bản dịch (không trùng).
    pub fn cau_can_dich(&self) -> Vec<String> {
        self.viec.cau_can_dich(self.dong.iter().map(|d| d.2.as_str()).filter(|c| c.chars().any(char::is_alphanumeric)))
    }

    fn chay_py(
        &self,
        cfg: serde_json::Value,
        dung: &AtomicBool,
        bao: &dyn Fn(&str),
        tien_do: &dyn Fn(&str, f64, f64),
    ) -> Result<PyKq> {
        let py = duong_dan::python().ok_or_else(|| anyhow!("chưa có Python riêng (Cài đặt > Tải công cụ)"))?;
        let ffmpeg = duong_dan::cong_cu().join("ffmpeg").join("bin").join("ffmpeg.exe");
        if !ffmpeg.is_file() {
            bail!("thiếu {}", ffmpeg.display());
        }
        let mo_hinh = thu_muc().join("_mo_hinh");
        std::fs::create_dir_all(&mo_hinh)?;
        let mut dll = vec![py.parent().unwrap().join("Lib").join("site-packages").join("torch").join("lib")];
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            dll.push(PathBuf::from(l).join("com.vieneu.app/gpu/venv/Lib/site-packages/torch/lib"));
        }
        let mut cfg = cfg;
        let o = cfg.as_object_mut().unwrap();
        o.insert("ffmpeg".into(), ffmpeg.to_string_lossy().into());
        o.insert("dll".into(), dll.iter().map(|d| d.to_string_lossy().into_owned()).collect::<Vec<_>>().into());
        o.insert("mo_hinh_dir".into(), mo_hinh.to_string_lossy().into());
        o.insert("deno_tim".into(), thu_muc().join("_thu_vien312").to_string_lossy().into());
        o.insert("gpu".into(), true.into());
        let vj = self.tm.join("viec.json");
        std::fs::write(&vj, serde_json::to_vec(&cfg)?)?;
        let kb = duong_dan::goc().join("scripts").join("nghe_phim.py");
        if !kb.is_file() {
            bail!("thiếu {}", kb.display());
        }
        let mut l = Command::new(py);
        l.arg("-I")
            .arg(&kb)
            .arg(&vj)
            .env("PYTHONIOENCODING", "utf-8")
            .env("PYTHONUTF8", "1")
            .env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .current_dir(&self.tm)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            l.creation_flags(0x0800_0000);
        }
        let mut con = l.spawn()?;
        let loi = con.stderr.take().map(|mut e| {
            std::thread::spawn(move || {
                let mut s = String::new();
                let _ = std::io::Read::read_to_string(&mut e, &mut s);
                s
            })
        });
        let stdout = con.stdout.take().unwrap();
        let con = std::sync::Mutex::new(con);
        let xong = AtomicBool::new(false);
        let mut kq = PyKq::default();
        let mut cuoi: Vec<String> = Vec::new();
        let mut nk = std::fs::OpenOptions::new().create(true).append(true).open(self.tm.join("nhat_ky.txt"))?;
        let mut bi_dung = false;
        std::thread::scope(|sc| {
            // Canh nút Dừng riêng: bấm Dừng là giết tiến trình ngay, không đợi nó in thêm dòng
            // (trước đây lúc tải / nạp mô hình im lặng thì bấm Dừng không có tác dụng).
            sc.spawn(|| {
                while !xong.load(Ordering::Relaxed) {
                    if dung.load(Ordering::Relaxed) {
                        let _ = con.lock().unwrap().kill();
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
            });
            for dong in BufReader::new(stdout).lines() {
                let d = dong.unwrap_or_default();
                use std::io::Write;
                let _ = writeln!(nk, "{d}");
                let (k, v) = d.split_once(' ').unwrap_or((d.as_str(), ""));
                let so = |s: &str| s.parse::<f64>().unwrap_or(0.0);
                match k {
                    "TAI" => tien_do("tai", so(v), 100.0),
                    "FILE" => kq.file = serde_json::from_str(v).ok(),
                    "CACH" => bao("Đang tải phim về..."),
                    "LOI_TAI" => bao("Cách tải này lỗi, thử cách khác..."),
                    "NAP_MO_HINH" => bao("Đang nạp AI nghe tiếng vào card màn hình..."),
                    "TAI_MO_HINH" => bao("Lần đầu: đang tải mô hình AI nghe tiếng (~1,6 GB), chờ vài phút..."),
                    "TIEN" => {
                        let mut t = v.split_whitespace();
                        tien_do("nghe", so(t.next().unwrap_or("0")), so(t.next().unwrap_or("0")));
                    }
                    "THIET_BI" => {
                        kq.thiet_bi = Some(v.to_string());
                        bao(if v == "cuda" { "Đang nghe tiếng bằng card màn hình..." } else { "Đang nghe tiếng bằng CPU (chậm hơn nhiều)..." });
                    }
                    "KHONG_CUDA" => kq.khong_cuda = true,
                    "GIAI_MA" => bao("Đang tách âm thanh khỏi phim..."),
                    "NGON_NGU" => kq.ngon_ngu = v.split_whitespace().next().map(String::from),
                    _ if !d.trim().is_empty() => {
                        cuoi.push(d.clone());
                        if cuoi.len() > 5 {
                            cuoi.remove(0);
                        }
                    }
                    _ => {}
                }
            }
            bi_dung = dung.load(Ordering::Relaxed);
            xong.store(true, Ordering::Relaxed);
        });
        let mut con = con.into_inner().unwrap();
        if bi_dung {
            let _ = con.kill();
            let _ = con.wait();
            bail!("đã dừng");
        }
        let st = con.wait()?;
        let loi = loi.and_then(|h| h.join().ok()).unwrap_or_default();
        if st.code() == Some(3) && kq.khong_cuda {
            return Ok(kq);
        }
        if st.code() == Some(4) {
            let l: Vec<&str> = loi.lines().rev().take(2).collect();
            bail!("không tải được phim từ link này (YouTube hay đổi cách chặn). Hãy tải phim về máy bằng cách khác rồi chọn file phim. Chi tiết: {}", l.into_iter().rev().collect::<Vec<_>>().join(" "));
        }
        if !st.success() {
            let l: Vec<&str> = loi.lines().rev().take(3).collect();
            bail!("công cụ nghe tiếng bị lỗi – xem {}\n{}\n{}", self.tm.join("nhat_ky.txt").display(), cuoi.join("\n"), l.into_iter().rev().collect::<Vec<_>>().join("\n"));
        }
        Ok(kq)
    }

    /// Tải phim (nếu là link) rồi nghe ra chữ. Đã có chữ rồi thì bỏ qua.
    pub fn tai_va_nghe(
        &mut self,
        ngon_ngu: Option<&str>,
        dung: &AtomicBool,
        bao: &dyn Fn(&str),
        tien_do: &dyn Fn(&str, f64, f64),
    ) -> Result<()> {
        if self.phim.is_none() {
            if !la_link(&self.nguon) {
                bail!("không thấy file phim");
            }
            let thu_tai = PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default()).join("Videos").join("DichGame");
            std::fs::create_dir_all(&thu_tai)?;
            bao("Đang tải phim về...");
            let kq = self.chay_py(
                serde_json::json!({"nguon": self.nguon, "tai": true, "chi_tai": true,
                    "thu_muc_tai": thu_tai.to_string_lossy(), "mo_hinh": "large-v3-turbo",
                    "ra": self.tm.join("chu_goc.json").to_string_lossy()}),
                dung, bao, tien_do,
            )?;
            let p = kq.file.ok_or_else(|| anyhow!("không tải được phim từ link này"))?;
            std::fs::write(self.tm.join("phim.json"), serde_json::to_vec(&p)?)?;
            self.phim = Some(p);
        }
        if !self.dong.is_empty() {
            return Ok(());
        }
        bao("Đang nạp AI nghe tiếng...");
        let phim = self.phim.clone().unwrap();
        let cfg = serde_json::json!({"nguon": phim.duong_dan.to_string_lossy(), "mo_hinh": "large-v3-turbo",
            "ngon_ngu": ngon_ngu, "ra": self.tm.join("chu_goc.json").to_string_lossy()});
        let mut kq = self.chay_py(cfg.clone(), dung, bao, tien_do)?;
        if kq.khong_cuda && kq.thiet_bi.is_none() {
            bao("Card màn hình chưa dùng được – chạy bằng CPU (chậm hơn nhiều)...");
            let mut c = cfg;
            c["cho_cpu"] = true.into();
            kq = self.chay_py(c, dung, bao, tien_do)?;
        }
        let _ = kq;
        self.nap_chu();
        if self.dong.is_empty() {
            bail!("không nghe được lời thoại nào trong phim");
        }
        Ok(())
    }

    /// Ghi .srt tiếng Việt cạnh phim (không ghi được thì vào thư mục dữ liệu). → đường dẫn.
    pub fn xuat_srt(&self) -> Result<PathBuf> {
        let phim = self.phim.as_ref().ok_or_else(|| anyhow!("chưa có phim"))?;
        let dong: Vec<Dong> = self
            .dong
            .iter()
            .map(|(a, b, c)| (*a, *b, self.viec.tim(c).filter(|v| !v.is_empty()).unwrap_or_else(|| c.clone())))
            .collect();
        let ten = phim.duong_dan.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let canh = phim.duong_dan.with_extension("srt");
        let trong = self.tm.join(format!("{ten}.srt"));
        let _ = std::fs::write(self.tm.join("goc.srt"), noi_dung_srt(&self.dong));
        for p in [canh, trong] {
            if std::fs::write(&p, noi_dung_srt(&dong)).is_ok() {
                return Ok(p);
            }
        }
        bail!("không ghi được file phụ đề")
    }

    /// Nhờ AI local soát bản dịch các dòng phụ đề rồi ghi lại .srt. → (số câu đã sửa, đường dẫn .srt)
    pub fn ai_sua_phu_de(&mut self, bc: &dyn crate::viet_hoa::viec_dich::BaoCao) -> Result<(usize, PathBuf)> {
        let mut thay: std::collections::BTreeSet<String> = Default::default();
        let mut cap: Vec<(String, String)> = Vec::new();
        for (_, _, en) in &self.dong {
            if !en.chars().any(char::is_alphanumeric) || !thay.insert(en.clone()) {
                continue;
            }
            if let Some(vi) = self.viec.tim(en).filter(|v| !v.is_empty()) {
                cap.push((en.clone(), vi));
            }
        }
        let n = self.viec.ai_sua(cap, bc)?;
        Ok((n, self.xuat_srt()?))
    }

    pub fn thu_muc_phim(&self) -> PathBuf {
        self.tm.clone()
    }
}

#[derive(Default)]
struct PyKq {
    file: Option<Phim>,
    thiet_bi: Option<String>,
    khong_cuda: bool,
    #[allow(dead_code)]
    ngon_ngu: Option<String>,
}

#[cfg(test)]
mod test {
    use super::*;

    fn tu(a: f64, b: f64, s: &str) -> Tu {
        Tu(a, b, s.into())
    }

    #[test]
    fn chia_va_srt() {
        let doan = vec![Doan {
            a: 0.0,
            b: 9.0,
            chu: String::new(),
            tu: vec![
                tu(0.0, 0.5, " Hello"), tu(0.5, 1.0, " there."),
                tu(1.0, 2.0, " How"), tu(2.0, 3.0, " are"), tu(3.0, 4.0, " you"), tu(4.0, 5.0, " doing"),
                tu(5.0, 6.0, " today?"),
            ],
        }];
        let d = chia_dong(&doan);
        assert!(!d.is_empty());
        assert_eq!(d[0].2.starts_with("Hello"), true);
        let srt = String::from_utf8(noi_dung_srt(&[(0.0, 1.5, "Xin chào".into()), (3661.5, 3663.0, "x".into())])).unwrap();
        assert!(srt.starts_with('\u{feff}'));
        assert!(srt.contains("00:00:00,000 --> 00:00:01,900"));
        assert!(srt.contains("01:01:01,500"));
        assert_eq!(xuong_dong("a b"), "a b");
        assert!(xuong_dong(&"abc ".repeat(15)).contains('\n'));
        assert!(la_link("  https://youtu.be/x") && !la_link("D:\\a.mp4"));
    }

    /// Nghe thật 1 file: FILE=<video/audio> cargo test --release -- --ignored nghe_that --nocapture
    #[test]
    #[ignore]
    fn nghe_that() {
        let f = std::env::var("FILE").unwrap();
        let mut v = ViecPhim::moi(&f).unwrap();
        let _ = std::fs::remove_file(v.tm.join("chu_goc.json"));
        v.dong.clear();
        let dung = AtomicBool::new(false);
        v.tai_va_nghe(None, &dung, &|s| println!("{s}"), &|b, a, t| println!("{b} {a}/{t}")).unwrap();
        println!("{} dòng, ngôn ngữ {:?}", v.dong.len(), v.ngon_ngu);
        for d in v.dong.iter().take(5) {
            println!("{:.1}-{:.1} {}", d.0, d.1, d.2);
        }
        assert!(!v.dong.is_empty());
    }
}
