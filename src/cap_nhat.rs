//! Cập nhật app từ GitHub Releases (https://github.com/Zinit-mid/Viet-hoa/releases):
//! kiểm tra bản mới → tải VietHoa.exe → đổi tên exe đang chạy thành .cu, đặt bản mới vào chỗ cũ → mở lại.
//! Không gửi dữ liệu người dùng; chỉ đọc thông tin phát hành công khai.

use std::cmp::Ordering;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

pub const KHO: &str = "Zinit-mid/Viet-hoa";
pub const PHIEN_BAN: &str = env!("CARGO_PKG_VERSION");
const TEN_EXE: &str = "VietHoa.exe";

#[derive(Clone, Debug)]
pub struct Ban {
    pub ma: String,
    pub ghi_chu: String,
    pub url: String,
    pub kich: u64,
}

/// "v0.2.1" / "0.2.1" → [0,2,1]
fn so_phien_ban(s: &str) -> Vec<u64> {
    s.trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|x| {
            x.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

pub fn moi_hon(a: &str, b: &str) -> bool {
    let (x, y) = (so_phien_ban(a), so_phien_ban(b));
    let n = x.len().max(y.len());
    (0..n)
        .map(|i| x.get(i).unwrap_or(&0).cmp(y.get(i).unwrap_or(&0)))
        .find(|o| *o != Ordering::Equal)
        == Some(Ordering::Greater)
}

/// Bản mới hơn bản đang chạy (None nếu đang là bản mới nhất).
pub fn kiem_tra() -> Result<Option<Ban>> {
    let url = format!("https://api.github.com/repos/{KHO}/releases/latest");
    let v: Value = ureq::get(&url)
        .set("User-Agent", "VietHoa-capnhat")
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => anyhow!("chưa có bản phát hành nào trên GitHub"),
            e => anyhow!("không kết nối được GitHub: {e}"),
        })?
        .into_json()?;
    let ma = v["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow!("phản hồi GitHub không có tag_name"))?
        .trim_start_matches(['v', 'V'])
        .to_string();
    if !moi_hon(&ma, PHIEN_BAN) {
        return Ok(None);
    }
    let tep = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|t| t["name"].as_str() == Some(TEN_EXE)))
        .ok_or_else(|| anyhow!("bản {ma} chưa đính kèm {TEN_EXE}"))?;
    Ok(Some(Ban {
        ma,
        ghi_chu: v["body"].as_str().unwrap_or("").to_string(),
        url: tep["browser_download_url"]
            .as_str()
            .ok_or_else(|| anyhow!("thiếu đường dẫn tải"))?
            .to_string(),
        kich: tep["size"].as_u64().unwrap_or(0),
    }))
}

fn duong_exe() -> Result<PathBuf> {
    Ok(std::env::current_exe()?)
}

/// Xóa file .cu còn sót của lần cập nhật trước (gọi lúc khởi động).
pub fn don_dep() {
    if let Ok(e) = duong_exe() {
        let _ = std::fs::remove_file(e.with_extension("exe.cu"));
        let _ = std::fs::remove_file(e.with_extension("exe.moi"));
    }
}

/// Tải bản mới và thay exe. `tien(đã tải, tổng)`. Xong thì cần khởi động lại.
pub fn tai_va_thay(ban: &Ban, tien: &dyn Fn(u64, u64)) -> Result<()> {
    let exe = duong_exe()?;
    let moi = exe.with_extension("exe.moi");
    let cu = exe.with_extension("exe.cu");
    let r = ureq::get(&ban.url)
        .set("User-Agent", "VietHoa-capnhat")
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| anyhow!("tải lỗi: {e}"))?;
    let tong = r
        .header("Content-Length")
        .and_then(|x| x.parse().ok())
        .unwrap_or(ban.kich);
    let mut doc = r.into_reader();
    let mut f = std::fs::File::create(&moi)?;
    let (mut buf, mut da) = (vec![0u8; 1 << 16], 0u64);
    let mut dau = [0u8; 2];
    loop {
        let n = doc.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if da == 0 && n >= 2 {
            dau.copy_from_slice(&buf[..2]);
        }
        f.write_all(&buf[..n])?;
        da += n as u64;
        tien(da, tong);
    }
    f.flush()?;
    drop(f);
    if da < 1_000_000 || &dau != b"MZ" || (ban.kich > 0 && da != ban.kich) {
        let _ = std::fs::remove_file(&moi);
        bail!("file tải về không hợp lệ ({da} byte) – thử lại");
    }
    let _ = std::fs::remove_file(&cu);
    std::fs::rename(&exe, &cu).map_err(|e| anyhow!("không đổi tên exe đang chạy: {e}"))?;
    if let Err(e) = std::fs::rename(&moi, &exe) {
        let _ = std::fs::rename(&cu, &exe); // trả lại bản cũ
        bail!("không đặt được bản mới: {e}");
    }
    Ok(())
}

/// Mở lại app sau ~2 giây (để tiến trình hiện tại kịp lưu dữ liệu và thoát).
pub fn mo_lai() {
    if let Ok(e) = duong_exe() {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .raw_arg(format!(
                "/c ping -n 3 127.0.0.1 >nul & start \"\" \"{}\"",
                e.display()
            ))
            .creation_flags(0x0800_0000)
            .spawn();
    }
}

#[derive(Default)]
pub struct TrangThai {
    pub dang_kiem: bool,
    pub ban: Option<Ban>,
    pub tb: String,
    pub loi: bool,
    pub dang_tai: bool,
    pub tien: f32,
    pub da_thay: bool,
}

#[derive(Clone, Default)]
pub struct CapNhat(pub Arc<Mutex<TrangThai>>);

impl CapNhat {
    pub fn dang_ky_tt(&self) {
        let c = self.0.clone();
        crate::tien_trinh::dang_ky("cap_nhat", None, move || {
            let t = c.lock().unwrap();
            crate::tien_trinh::Anh {
                dang_ban: t.dang_tai,
                ten: "Cập nhật app".into(),
                thong_bao: t.tb.clone(),
                tien: (t.tien > 0.0).then_some(t.tien),
                loi: t.loi,
            }
        });
    }

    /// Kiểm tra bản mới trong luồng nền. `im_lang`: không báo gì khi đã là bản mới nhất / lỗi mạng.
    pub fn kiem_tra_nen(&self, im_lang: bool) {
        {
            let mut t = self.0.lock().unwrap();
            if t.dang_kiem || t.dang_tai {
                return;
            }
            t.dang_kiem = true;
            if !im_lang {
                (t.tb, t.loi) = ("Đang kiểm tra bản mới...".into(), false);
            }
        }
        let c = self.0.clone();
        std::thread::spawn(move || {
            let kq = kiem_tra();
            let mut t = c.lock().unwrap();
            t.dang_kiem = false;
            match kq {
                Ok(Some(b)) => {
                    t.tb = format!("Có bản mới {} (đang chạy {PHIEN_BAN}).", b.ma);
                    t.loi = false;
                    t.ban = Some(b);
                }
                Ok(None) => {
                    t.ban = None;
                    if !im_lang {
                        (t.tb, t.loi) = (format!("Đang là bản mới nhất ({PHIEN_BAN})."), false);
                    }
                }
                Err(e) => {
                    if !im_lang {
                        (t.tb, t.loi) = (format!("Lỗi: {e}"), true);
                    }
                }
            }
        });
    }

    pub fn cap_nhat_nen(&self) {
        let Some(ban) = ({
            let mut t = self.0.lock().unwrap();
            if t.dang_tai {
                return;
            }
            t.dang_tai = true;
            (t.tien, t.loi, t.tb) = (
                0.0,
                false,
                format!(
                    "Đang tải bản {}...",
                    t.ban.as_ref().map(|b| b.ma.as_str()).unwrap_or("")
                ),
            );
            t.ban.clone()
        }) else {
            return;
        };
        let c = self.0.clone();
        std::thread::spawn(move || {
            let c2 = c.clone();
            let kq = tai_va_thay(&ban, &move |a, b| {
                if b > 0 {
                    c2.lock().unwrap().tien = a as f32 / b as f32;
                }
            });
            let mut t = c.lock().unwrap();
            t.dang_tai = false;
            match kq {
                Ok(()) => {
                    (t.tb, t.da_thay) = (
                        format!("Đã cập nhật lên {}. Bấm Khởi động lại.", ban.ma),
                        true,
                    );
                    t.ban = None; // ẩn dòng "Có bản mới" ở thanh bên
                }
                Err(e) => (t.tb, t.loi) = (format!("Lỗi: {e}"), true),
            }
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn so_sanh_phien_ban() {
        assert!(moi_hon("v0.2.0", "0.1.9"));
        assert!(moi_hon("1.0", "0.9.9"));
        assert!(moi_hon("0.1.10", "0.1.9"));
        assert!(!moi_hon("0.1.0", "0.1.0"));
        assert!(!moi_hon("0.1", "0.1.0"));
        assert!(!moi_hon("0.0.9", "0.1.0"));
    }
}
