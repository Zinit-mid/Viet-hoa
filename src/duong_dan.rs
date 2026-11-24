//! Thư mục gốc của app. Mọi dữ liệu nằm cạnh app (mặc định D:\Việt hóa),
//! để cài lại Windows (ổ C) không mất gì.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static GOC: OnceLock<PathBuf> = OnceLock::new();

const SCRIPTS: [(&str, &str); 6] = [
    ("dich_offline.py", include_str!("../scripts/dich_offline.py")),
    ("tao_giong.py", include_str!("../scripts/tao_giong.py")),
    ("long_tieng_tr.py", include_str!("../scripts/long_tieng_tr.py")),
    ("nghe_phim.py", include_str!("../scripts/nghe_phim.py")),
    ("thuyet_minh.py", include_str!("../scripts/thuyet_minh.py")),
    ("tai_cong_cu.ps1", include_str!("../scripts/tai_cong_cu.ps1")),
];

/// Bản chạy độc lập (exe chép sang máy khác): chép các script nhúng trong exe ra `scripts\` cạnh exe.
/// Khi chạy từ mã nguồn (có Cargo.toml) thì dùng thẳng thư mục scripts của dự án.
pub fn chep_script_nhung() {
    let g = goc();
    if g.join("Cargo.toml").exists() {
        return;
    }
    let d = g.join("scripts");
    let _ = fs::create_dir_all(&d);
    for (ten, nd) in SCRIPTS {
        let p = d.join(ten);
        if fs::read_to_string(&p).map(|c| c.replace("\r\n", "\n") != nd.replace("\r\n", "\n")).unwrap_or(true) {
            let _ = fs::write(&p, nd);
        }
    }
}

/// Thư mục gốc: nơi chứa VietHoa.exe. Khi chạy từ mã nguồn (target\debug)
/// thì lấy thư mục có Cargo.toml.
pub fn goc() -> &'static Path {
    GOC.get_or_init(|| {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let mut d = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        for _ in 0..8 {
            if d.join("Cargo.toml").exists() {
                return d;
            }
            match d.parent() {
                Some(p) => d = p.to_path_buf(),
                None => break,
            }
        }
        let ben_exe = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        // Exe được chép ra chỗ khác (Desktop...) → dùng thư mục cài đặt chính
        // để có chung dữ liệu, công cụ, mô hình dịch.
        if !ben_exe.join("cong_cu").is_dir() {
            if let Some(chinh) = thu_muc_cai_dat().filter(|p| p.join("cong_cu").is_dir()) {
                return chinh;
            }
        }
        ben_exe
    })
}

/// Thư mục cài đặt chính: biến môi trường VIET_HOA_GOC, nếu không có thì D:\Việt hóa.
fn thu_muc_cai_dat() -> Option<PathBuf> {
    std::env::var_os("VIET_HOA_GOC")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(r"D:\Việt hóa")))
}

/// Tạo lối tắt "Việt Hóa" trên Desktop trỏ tới exe trong thư mục cài đặt.
pub fn tao_loi_tat_desktop() -> Result<PathBuf, String> {
    use std::os::windows::process::CommandExt;
    let exe = goc().join("VietHoa.exe");
    let exe = if exe.exists() {
        exe
    } else {
        std::env::current_exe().map_err(|e| e.to_string())?
    };
    let lenh = format!(
        "$d=[Environment]::GetFolderPath('Desktop'); $s=(New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $d 'Việt Hóa.lnk')); $s.TargetPath='{}'; $s.WorkingDirectory='{}'; $s.Save(); Join-Path $d 'Việt Hóa.lnk'",
        exe.display().to_string().replace('\'', "''"),
        goc().display().to_string().replace('\'', "''"),
    );
    let ra = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &lenh])
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| e.to_string())?;
    if !ra.status.success() {
        return Err(String::from_utf8_lossy(&ra.stderr).trim().to_string());
    }
    Ok(PathBuf::from(String::from_utf8_lossy(&ra.stdout).trim()))
}

pub fn du_lieu() -> PathBuf {
    // VIET_HOA_DU_LIEU: thư mục dữ liệu khác (dùng khi kiểm thử)
    std::env::var_os("VIET_HOA_DU_LIEU")
        .map(PathBuf::from)
        .unwrap_or_else(|| goc().join("du_lieu"))
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
        goc()
            .join("DichGame_AI")
            .join("mo_hinh_dich_offline")
            .join("quickmt-en-vi"),
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
    if fs::metadata(&p)
        .map(|m| m.len() > 1_000_000)
        .unwrap_or(false)
    {
        let _ = fs::rename(&p, du_lieu().join("nhat_ky_cu.txt"));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&p) {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
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

/// Giữ máy không ngủ trong lúc làm việc dài (quét / dịch / cài / lồng tiếng); hết phạm vi thì trả lại.
pub struct GiuMayThuc;

impl GiuMayThuc {
    pub fn moi() -> Self {
        use windows::Win32::System::Power::{
            SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
        };
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
        }
        GiuMayThuc
    }
}

impl Drop for GiuMayThuc {
    fn drop(&mut self) {
        use windows::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}

/// Giờ máy "dd/mm/yyyy HH:MM" (giống time.strftime của app cũ).
pub fn gio_dia_phuong() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!(
        "{:02}/{:02}/{} {:02}:{:02}",
        t.wDay, t.wMonth, t.wYear, t.wHour, t.wMinute
    )
}

/// Mở Explorer: thư mục, hoặc thư mục chứa file và chọn sẵn file đó.
pub fn mo_explorer(p: &Path) {
    use std::os::windows::process::CommandExt;
    let mut c = std::process::Command::new("explorer");
    if p.is_file() {
        c.raw_arg(format!("/select,\"{}\"", p.display()));
    } else {
        c.arg(p);
    }
    let _ = c.spawn();
}

/// Desktop thật người dùng đang thấy (kể cả khi OneDrive chuyển Desktop).
pub fn desktop() -> PathBuf {
    for p in [
        std::env::var("OneDrive")
            .ok()
            .map(|o| PathBuf::from(o).join("Desktop")),
        std::env::var("USERPROFILE")
            .ok()
            .map(|u| PathBuf::from(u).join("Desktop")),
    ]
    .into_iter()
    .flatten()
    {
        if p.is_dir() {
            return p;
        }
    }
    du_lieu()
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
        if bo
            .iter()
            .any(|b| ten.to_string_lossy().eq_ignore_ascii_case(b))
        {
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
