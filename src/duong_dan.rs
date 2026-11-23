//! Thư mục gốc của app. Mọi dữ liệu nằm cạnh app (mặc định D:\Việt hóa),
//! để cài lại Windows (ổ C) không mất gì.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static GOC: OnceLock<PathBuf> = OnceLock::new();

/// Thư mục gốc: nơi chứa VietHoa.exe. Khi chạy từ mã nguồn (target\debug)
/// thì lấy thư mục có Cargo.toml.
pub fn goc() -> &'static Path {
    GOC.get_or_init(|| {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let mut d = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        for _ in 0..4 {
            if d.join("Cargo.toml").exists() {
                return d;
            }
            match d.parent() {
                Some(p) => d = p.to_path_buf(),
                None => break,
            }
        }
        exe.parent().map(Path::to_path_buf).unwrap_or_default()
    })
}

pub fn du_lieu() -> PathBuf {
    goc().join("du_lieu")
}
pub fn cong_cu() -> PathBuf {
    goc().join("cong_cu")
}
pub fn viet_hoa_game() -> PathBuf {
    du_lieu().join("viet_hoa_game")
}
pub fn file_du_lieu(ten: &str) -> PathBuf {
    du_lieu().join(ten)
}

/// Thư mục mô hình dịch offline: ưu tiên du_lieu\mo_hinh, rồi DichGame_AI cũ.
pub fn mo_hinh_offline() -> Option<PathBuf> {
    [
        du_lieu().join("mo_hinh").join("quickmt-en-vi"),
        goc().join("DichGame_AI").join("mo_hinh_dich_offline").join("quickmt-en-vi"),
    ]
    .into_iter()
    .find(|p| p.join("model.bin").exists())
}

pub fn python() -> Option<PathBuf> {
    let p = cong_cu().join("python").join("python.exe");
    p.exists().then_some(p)
}

/// Ghi một dòng vào du_lieu\nhat_ky.txt (giữ tối đa ~1 MB).
pub fn nhat_ky(dong: &str) {
    use std::io::Write;
    let p = du_lieu().join("nhat_ky.txt");
    if fs::metadata(&p).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = fs::rename(&p, du_lieu().join("nhat_ky_cu.txt"));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&p) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let giay = t.as_secs() % 86_400;
        let _ = writeln!(
            f,
            "{:02}:{:02}:{:02}.{:03} {dong}",
            (giay / 3600 + 7) % 24,
            giay / 60 % 60,
            giay % 60,
            t.subsec_millis()
        );
    }
}

/// Ghi file an toàn: ghi .tam rồi đổi tên.
pub fn ghi_file(duong: &Path, noi_dung: &[u8]) -> std::io::Result<()> {
    if let Some(cha) = duong.parent() {
        fs::create_dir_all(cha)?;
    }
    let tam = duong.with_extension(format!(
        "{}.tam",
        duong.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    fs::write(&tam, noi_dung)?;
    fs::rename(&tam, duong)
}

/// Lần đầu chạy: chép dữ liệu nhỏ từ app Python cũ (%APPDATA%) sang du_lieu.
/// Chỉ CHÉP (không xóa) để app cũ vẫn chạy được. Bỏ qua mô hình AI (đã ở DichGame_AI).
pub fn chuyen_du_lieu_cu() -> Vec<String> {
    let mut bao = Vec::new();
    let dau = du_lieu().join("da_chuyen_du_lieu_cu.txt");
    if dau.exists() {
        return bao;
    }
    let _ = fs::create_dir_all(du_lieu());
    if let Ok(appdata) = std::env::var("APPDATA") {
        let cu = Path::new(&appdata);
        let viec = [
            (cu.join("DichGame"), du_lieu(), vec!["mo_hinh"]),
            (cu.join("VietHoaGame"), viet_hoa_game(), vec!["phim"]),
        ];
        for (nguon, dich, bo) in viec {
            if nguon.is_dir() {
                match chep_thu_muc(&nguon, &dich, &bo) {
                    Ok(n) => bao.push(format!("Đã chép {n} file từ {}", nguon.display())),
                    Err(e) => bao.push(format!("Lỗi chép {}: {e}", nguon.display())),
                }
            }
        }
    }
    let _ = fs::write(&dau, bao.join("\r\n"));
    bao
}

fn chep_thu_muc(nguon: &Path, dich: &Path, bo: &[&str]) -> std::io::Result<usize> {
    fs::create_dir_all(dich)?;
    let mut n = 0;
    for muc in fs::read_dir(nguon)? {
        let muc = muc?;
        let ten = muc.file_name();
        if bo.iter().any(|b| ten.to_string_lossy().eq_ignore_ascii_case(b)) {
            continue;
        }
        let den = dich.join(&ten);
        let loai = muc.file_type()?;
        if loai.is_dir() {
            n += chep_thu_muc(&muc.path(), &den, &[])?;
        } else if loai.is_file() && !den.exists() {
            fs::copy(muc.path(), &den)?;
            n += 1;
        }
    }
    Ok(n)
}
