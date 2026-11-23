//! Dịch offline: chạy scripts\dich_offline.py bằng Python riêng (cong_cu\python),
//! giữ tiến trình sống, gửi / nhận từng dòng JSON. Chỉ dịch tiếng Anh.

use anyhow::{anyhow, bail, Result};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;

use crate::duong_dan;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

struct TienTrinh {
    _con: Child,
    vao: ChildStdin,
    ra: BufReader<ChildStdout>,
}

static TT: Mutex<Option<TienTrinh>> = Mutex::new(None);

pub fn co_the_dung() -> bool {
    duong_dan::python().is_some() && duong_dan::mo_hinh_offline().is_some()
}

fn khoi_dong() -> Result<TienTrinh> {
    let py = duong_dan::python().ok_or_else(|| anyhow!("Chưa có Python riêng (cong_cu\\python)"))?;
    let mh = duong_dan::mo_hinh_offline().ok_or_else(|| anyhow!("Chưa có mô hình quickmt-en-vi"))?;
    let script = duong_dan::goc().join("scripts").join("dich_offline.py");
    let mut con = Command::new(py)
        .arg("-I")
        .arg(script)
        .arg(mh)
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    let vao = con.stdin.take().ok_or_else(|| anyhow!("không mở được stdin"))?;
    let mut ra = BufReader::new(con.stdout.take().ok_or_else(|| anyhow!("không mở được stdout"))?);
    let mut dong = String::new();
    ra.read_line(&mut dong)?;
    if !dong.starts_with("SAN_SANG") {
        bail!("Dịch offline không khởi động được");
    }
    Ok(TienTrinh { _con: con, vao, ra })
}

pub fn dich_lo(cac_cau: &[String]) -> Result<Vec<String>> {
    let mut khoa = TT.lock().map_err(|_| anyhow!("khóa hỏng"))?;
    if khoa.is_none() {
        *khoa = Some(khoi_dong()?);
    }
    let tt = khoa.as_mut().unwrap();
    let kq = (|| -> Result<Vec<String>> {
        writeln!(tt.vao, "{}", serde_json::to_string(cac_cau)?)?;
        tt.vao.flush()?;
        let mut dong = String::new();
        if tt.ra.read_line(&mut dong)? == 0 {
            bail!("Tiến trình dịch offline đã tắt");
        }
        let v: serde_json::Value = serde_json::from_str(&dong)?;
        if let Some(loi) = v.get("loi") {
            bail!("Dịch offline lỗi: {loi}");
        }
        Ok(serde_json::from_value(v)?)
    })();
    if kq.is_err() {
        *khoa = None; // lần sau khởi động lại
    }
    kq
}

/// Nạp sẵn mô hình ở luồng nền (lúc bật dịch) để câu đầu tiên không phải chờ.
pub fn nap_san() {
    if !co_the_dung() {
        return;
    }
    std::thread::spawn(|| {
        if let Ok(mut k) = TT.lock() {
            if k.is_none() {
                *k = khoi_dong().ok();
            }
        }
    });
}

pub fn dich(chu: &str) -> Result<String> {
    Ok(dich_lo(&[chu.to_string()])?.pop().unwrap_or_default())
}

pub fn tat() {
    if let Ok(mut k) = TT.lock() {
        if let Some(mut t) = k.take() {
            let _ = t._con.kill();
        }
    }
}
