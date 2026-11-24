//! Gói .tiger của engine Foundation (Tomb Raider 2013 / Rise / Shadow), locals.bin, .nfo.
//! Chuyển từ viet_hoa_rottr.py + viet_hoa_tr.py của app cũ (định dạng theo TR Reboot Mod Tools, MIT).
//!   v3 (TR2013): bảng 16 byte, băm 32 bit, vị trí gói gộp trong 1 số
//!   v4 (Rise):   bảng 24 byte, băm 32 bit
//!   v5 (Shadow): bảng 32 byte, băm 64 bit, có "sub"
//! Dữ liệu có thể nén khối CDRM (zlib từng khúc).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use unicode_normalization::UnicodeNormalization;

pub const MAGIC_TIGER: u32 = 0x5346_4154;
pub const MAGIC_CDRM: u32 = 0x4D52_4443;

/// CRC32 kiểu Crystal Dynamics (MSB trước, đa thức 0x04C11DB7).
pub fn bam_cdc(s: &str) -> u32 {
    let mut h: u32 = 0xFFFF_FFFF;
    for c in s.chars() {
        h ^= (c as u32) << 24;
        for _ in 0..8 {
            h = if h & 0x8000_0000 != 0 {
                (h << 1) ^ 0x04C1_1DB7
            } else {
                h << 1
            };
        }
    }
    !h
}

/// FNV-1a 64 bit (Shadow).
pub fn bam64(s: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for c in s.chars() {
        h = (h ^ c as u64).wrapping_mul(0x100_0000_01B3);
    }
    h
}

#[derive(Clone, Debug)]
pub struct DauTiger {
    pub ban: i32,
    pub id: i32,
    pub sub: i32,
    pub so_phan: i32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Muc {
    pub bam: u64,
    pub locale: u64,
    pub co: u32,
    pub phan: i32,
    pub id: i32,
    #[serde(default)]
    pub sub: i32,
    pub vt: u32,
}

fn i32_at(d: &[u8], o: usize) -> Result<i32> {
    d.get(o..o + 4)
        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| anyhow!("file hỏng"))
}
fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    Ok(i32_at(d, o)? as u32)
}
fn u64_at(d: &[u8], o: usize) -> Result<u64> {
    d.get(o..o + 8)
        .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| anyhow!("file hỏng"))
}

/// Đọc đầu + bảng file của gói .tiger (v3 / v4 / v5).
pub fn doc_tiger(p: &Path) -> Result<(DauTiger, Vec<Muc>)> {
    let mut f = File::open(p)?;
    let mut d = vec![0u8; 0x38];
    let n = f.read(&mut d)?;
    if n < 0x34 {
        bail!("file .tiger quá ngắn");
    }
    if u32_at(&d, 0)? != MAGIC_TIGER {
        bail!("không phải file .tiger");
    }
    let ban = i32_at(&d, 4)?;
    let so_phan = i32_at(&d, 8)?;
    let so_tep = i32_at(&d, 12)?.max(0) as usize;
    let id = i32_at(&d, 16)?;
    let (co_dau, kich) = match ban {
        3 => (0x34, 16),
        4 => (0x34, 24),
        5 => (0x38, 32),
        _ => bail!("file .tiger phiên bản {ban} chưa hỗ trợ"),
    };
    let sub = if ban == 5 { i32_at(&d, 0x14)? } else { 0 };
    f.seek(SeekFrom::Start(co_dau))?;
    let mut bang = vec![0u8; kich * so_tep];
    f.read_exact(&mut bang)?;
    let mut ds = Vec::with_capacity(so_tep);
    for i in 0..so_tep {
        let o = kich * i;
        let b = &bang;
        let h16 = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]) as i32;
        ds.push(match ban {
            3 => {
                let goi = u32_at(b, o + 12)?;
                Muc {
                    bam: u32_at(b, o)? as u64,
                    locale: u32_at(b, o + 4)? as u64,
                    co: u32_at(b, o + 8)?,
                    phan: (goi & 0xF) as i32,
                    id: ((goi >> 4) & 0x7F) as i32,
                    sub: 0,
                    vt: goi & 0xFFFF_F800,
                }
            }
            4 => Muc {
                bam: u32_at(b, o)? as u64,
                locale: u32_at(b, o + 4)? as u64,
                co: u32_at(b, o + 8)?,
                phan: h16(o + 16),
                id: h16(o + 18),
                sub: 0,
                vt: u32_at(b, o + 20)?,
            },
            _ => Muc {
                bam: u64_at(b, o)?,
                locale: u64_at(b, o + 8)?,
                co: u32_at(b, o + 16)?,
                phan: h16(o + 24),
                id: b[o + 26] as i32,
                sub: b[o + 27] as i32,
                vt: u32_at(b, o + 28)?,
            },
        });
    }
    Ok((
        DauTiger {
            ban,
            id,
            sub,
            so_phan,
        },
        ds,
    ))
}

/// Đường dẫn phần thứ `phan` của gói (xxx.000.tiger → xxx.001.tiger).
pub fn duong_dan_phan(p: &Path, phan: i32) -> PathBuf {
    if phan == 0 {
        return p.to_path_buf();
    }
    let ten = p
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let moi = match ten.rfind(".000.tiger") {
        Some(i) => format!("{}.{phan:03}.tiger", &ten[..i]),
        None => ten,
    };
    p.with_file_name(moi)
}

/// Nội dung 1 file trong gói (giải nén CDRM). tho=true: lấy nguyên byte trong gói (cả khối nén).
pub fn doc_du_lieu(p: &Path, muc: &Muc, tho: bool) -> Result<Vec<u8>> {
    let mut f = File::open(duong_dan_phan(p, muc.phan))?;
    doc_tu_file(&mut f, muc.vt as u64, muc.co as usize, tho)
}

pub fn doc_tu_file(f: &mut File, vt: u64, co: usize, tho: bool) -> Result<Vec<u8>> {
    f.seek(SeekFrom::Start(vt))?;
    let mut dau = [0u8; 16];
    let n = f.read(&mut dau)?;
    if n >= 16 && u32_at(&dau, 0)? == MAGIC_CDRM {
        let so = i32_at(&dau, 8)?;
        if !(0..100_000).contains(&so) {
            bail!("khối nén hỏng");
        }
        let so = so as usize;
        let mut bang = vec![0u8; 8 * so];
        f.read_exact(&mut bang)?;
        let mut q = (vt + 16 + 8 * so as u64 + 0xF) & !0xF;
        let mut ra = Vec::new();
        for i in 0..so {
            let goc = (u32_at(&bang, 8 * i)? >> 8) as usize;
            let nen = u32_at(&bang, 8 * i + 4)? as usize;
            f.seek(SeekFrom::Start(q))?;
            let mut b = vec![0u8; nen];
            f.read_exact(&mut b)?;
            if nen == goc {
                ra.extend_from_slice(&b);
            } else {
                let mut z = flate2::read::ZlibDecoder::new(&b[..]);
                z.read_to_end(&mut ra)?;
            }
            q = (q + nen as u64 + 0xF) & !0xF;
        }
        if tho {
            f.seek(SeekFrom::Start(vt))?;
            let mut b = vec![0u8; (q - vt) as usize];
            f.read_exact(&mut b)?;
            return Ok(b);
        }
        return Ok(ra);
    }
    f.seek(SeekFrom::Start(vt))?;
    let mut b = vec![0u8; co];
    f.read_exact(&mut b)?;
    Ok(b)
}

/// Nén thành khối CDRM (mỗi khúc ≤ 256 KB) giống game.
pub fn khoi_cdrm(du: &[u8]) -> Vec<u8> {
    let khuc: Vec<&[u8]> = if du.is_empty() {
        vec![&[][..]]
    } else {
        du.chunks(0x40000).collect()
    };
    let mut ra = Vec::new();
    ra.extend(MAGIC_CDRM.to_le_bytes());
    ra.extend(0i32.to_le_bytes());
    ra.extend((khuc.len() as i32).to_le_bytes());
    ra.extend(0u32.to_le_bytes());
    let vt0 = (16 + 8 * khuc.len() + 0xF) & !0xF;
    let mut bang = Vec::new();
    let mut than = Vec::new();
    for k in khuc {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(9));
        e.write_all(k).unwrap();
        let mut z = e.finish().unwrap();
        let mut loai = 2u32;
        if z.len() >= k.len() {
            z = k.to_vec();
            loai = 1;
        }
        bang.extend((((k.len() as u32) << 8) | loai).to_le_bytes());
        bang.extend((z.len() as u32).to_le_bytes());
        than.extend(z);
        than.resize(than.len() + ((16 - (vt0 + than.len()) % 16) % 16), 0);
    }
    ra.extend(bang);
    ra.resize(vt0, 0);
    ra.extend(than);
    ra
}

// ---------------- .nfo ----------------

#[derive(Default, Clone, Debug)]
pub struct Nfo {
    pub so: std::collections::HashMap<String, i64>,
    pub custom: Vec<String>,
}

impl Nfo {
    pub fn lay(&self, k: &str) -> Option<i64> {
        self.so.get(k).copied()
    }
}

pub fn doc_nfo(p: &Path) -> Result<Nfo> {
    let s = String::from_utf8_lossy(&std::fs::read(p)?).to_string();
    let dong: Vec<&str> = s.lines().map(str::trim).collect();
    let mut nfo = Nfo::default();
    if let Some(d) = dong.first() {
        let t: Vec<&str> = d.split(' ').collect();
        let mut i = 1;
        while i + 1 < t.len() {
            if let Ok(v) = t[i + 1].parse() {
                nfo.so.insert(t[i].to_string(), v);
            }
            i += 2;
        }
    }
    for x in dong.iter().skip(1) {
        if let Some(c) = x.strip_prefix("custom ") {
            nfo.custom.push(c.to_string());
        }
    }
    Ok(nfo)
}

pub fn ghi_nfo(p: &Path, bap: i64, phien_ban: i64, ma: i64, steam: Option<&str>) -> Result<()> {
    let ten = p.file_name().unwrap_or_default().to_string_lossy();
    let ten = ten.replacen("bigfile.", "", 1);
    let ten = ten.strip_suffix(".nfo").unwrap_or(&ten).replace(".000", "");
    let mut dong = vec![format!(
        "{ten} bap {bap} package {ma} chunk 0 version {phien_ban} required 1 bigfile 1 dlcindex {ma} mount 0 unmount 1"
    )];
    if let Some(s) = steam {
        dong.push(format!("custom {s}"));
    }
    dong.push("custom mod:Viet hoa (Dich Game)".into());
    std::fs::write(p, dong.join("\r\n") + "\r\n")?;
    Ok(())
}

// ---------------- ghi gói ----------------

/// File cần ghi: (băm, locale, hàm lấy dữ liệu) – lấy lần lượt khi ghi để không giữ hết trong RAM.
pub type TepGhi<'a> = (u64, u64, Box<dyn FnOnce() -> Result<Vec<u8>> + 'a>);

fn ghi_tam(dich: &Path) -> PathBuf {
    let mut s = dich.as_os_str().to_owned();
    s.push(".tam");
    PathBuf::from(s)
}

/// Gói v4 (Rise) 1 phần, ghi dần ra đĩa. BẢNG PHẢI XẾP THEO (băm, locale) – game tìm nhị phân.
pub fn tao_tiger_v4(
    duong_dan: &Path,
    ma: i32,
    mut cac_tep: Vec<TepGhi>,
    bao: &dyn Fn(&str),
) -> Result<()> {
    cac_tep.sort_by_key(|x| (x.0, x.1));
    let mut dau = Vec::new();
    for v in [MAGIC_TIGER as i32, 4, 1, cac_tep.len() as i32, ma] {
        dau.extend(v.to_le_bytes());
    }
    let mut ten = b"pcx64-w".to_vec();
    ten.resize(0x20, 0);
    dau.extend(ten);
    let mut bang = vec![0u8; 24 * cac_tep.len()];
    let tam = ghi_tam(duong_dan);
    let tong = cac_tep.len();
    {
        let mut f = std::io::BufWriter::new(File::create(&tam)?);
        f.write_all(&dau)?;
        f.write_all(&bang)?;
        let mut vt = (dau.len() + bang.len()) as u64;
        for (i, (bam, loc, ham)) in cac_tep.into_iter().enumerate() {
            let dem = (16 - vt % 16) % 16;
            f.write_all(&vec![0u8; dem as usize])?;
            vt += dem;
            let du = ham()?;
            let o = 24 * i;
            bang[o..o + 4].copy_from_slice(&(bam as u32).to_le_bytes());
            bang[o + 4..o + 8].copy_from_slice(&(loc as u32).to_le_bytes());
            bang[o + 8..o + 12].copy_from_slice(&(du.len() as u32).to_le_bytes());
            bang[o + 18..o + 20].copy_from_slice(&(ma as i16).to_le_bytes());
            bang[o + 20..o + 24].copy_from_slice(&(vt as u32).to_le_bytes());
            f.write_all(&du)?;
            vt += du.len() as u64;
            if vt > 0x7FFF_FFF0 {
                bail!("gói Việt hóa vượt 2 GB");
            }
            if i % 200 == 0 {
                bao(&format!("Đang ghi gói Việt hóa: {i}/{tong} file"));
            }
        }
        f.seek(SeekFrom::Start(dau.len() as u64))?;
        f.write_all(&bang)?;
        f.flush()?;
    }
    std::fs::rename(&tam, duong_dan)?;
    Ok(())
}

/// Gói v5 (Shadow) nhỏ, 1 phần.
pub fn tao_tiger_v5(
    duong_dan: &Path,
    ma: i32,
    sub: i32,
    mut cac_tep: Vec<(u64, u64, Vec<u8>)>,
) -> Result<()> {
    cac_tep.sort_by_key(|x| (x.0, x.1));
    let mut ra = Vec::new();
    for v in [MAGIC_TIGER as i32, 5, 1, cac_tep.len() as i32, ma, sub] {
        ra.extend(v.to_le_bytes());
    }
    let mut ten = b"pcx64-w".to_vec();
    ten.resize(0x20, 0);
    ra.extend(ten);
    let vt0 = ra.len() + 32 * cac_tep.len();
    let mut bang = Vec::new();
    let mut than: Vec<u8> = Vec::new();
    for (bam, loc, du) in &cac_tep {
        than.resize(than.len() + (16 - (vt0 + than.len()) % 16) % 16, 0);
        bang.extend(bam.to_le_bytes());
        bang.extend(loc.to_le_bytes());
        bang.extend((du.len() as u32).to_le_bytes());
        bang.extend(0u32.to_le_bytes());
        bang.extend(0i16.to_le_bytes());
        bang.push(ma as u8);
        bang.push(sub as u8);
        bang.extend(((vt0 + than.len()) as u32).to_le_bytes());
        than.extend(du);
    }
    ra.extend(bang);
    ra.extend(than);
    crate::duong_dan::ghi_file(duong_dan, &ra)?;
    Ok(())
}

/// Hàng của gói v3: (băm, locale, cỡ, con trỏ gói cũ | None, hàm lấy dữ liệu | None)
pub type HangV3<'a> = (
    u64,
    u64,
    u32,
    Option<u32>,
    Option<Box<dyn FnOnce() -> Result<Vec<u8>> + 'a>>,
);

/// Gói .tiger v3 (TR2013), ghi ra file tạm rồi thay. File nằm ở gói khác giữ nguyên con trỏ.
pub fn tao_tiger_v3(
    duong_dan: &Path,
    ma: i32,
    mut hang: Vec<HangV3>,
    bao: &dyn Fn(&str),
) -> Result<()> {
    hang.sort_by_key(|x| (x.0, x.1));
    let tong = hang.len();
    let mut dau = Vec::new();
    for v in [MAGIC_TIGER as i32, 3, 1, tong as i32, ma] {
        dau.extend(v.to_le_bytes());
    }
    let mut ten = b"pc-w".to_vec();
    ten.resize(0x20, 0);
    dau.extend(ten);
    let mut bang = vec![0u8; 16 * tong];
    let mut tam = duong_dan.as_os_str().to_owned();
    tam.push(".viethoa_tam");
    let tam = PathBuf::from(tam);
    {
        let mut f = std::io::BufWriter::new(File::create(&tam)?);
        f.write_all(&dau)?;
        f.write_all(&bang)?;
        let mut vt = (dau.len() + bang.len()) as u64;
        for (i, (bam, loc, mut co, goi, ham)) in hang.into_iter().enumerate() {
            let goi = match (goi, ham) {
                (Some(g), _) => g,
                (None, Some(ham)) => {
                    let dem = (0x800 - vt % 0x800) % 0x800;
                    f.write_all(&vec![0u8; dem as usize])?;
                    vt += dem;
                    let du = ham()?;
                    if vt + du.len() as u64 > 0xFFFF_F800 {
                        bail!("gói vượt 4 GB");
                    }
                    let g = vt as u32 | ((ma as u32) << 4);
                    f.write_all(&du)?;
                    vt += du.len() as u64;
                    if co == 0 {
                        co = du.len() as u32;
                    }
                    g
                }
                _ => bail!("hàng gói thiếu dữ liệu"),
            };
            let o = 16 * i;
            bang[o..o + 4].copy_from_slice(&(bam as u32).to_le_bytes());
            bang[o + 4..o + 8].copy_from_slice(&(loc as u32).to_le_bytes());
            bang[o + 8..o + 12].copy_from_slice(&co.to_le_bytes());
            bang[o + 12..o + 16].copy_from_slice(&goi.to_le_bytes());
            if i % 300 == 0 {
                bao(&format!("Đang ghi gói Việt hóa: {i}/{tong} file"));
            }
        }
        f.write_all(&vec![0u8; ((0x800 - vt % 0x800) % 0x800) as usize])?;
        f.seek(SeekFrom::Start(dau.len() as u64))?;
        f.write_all(&bang)?;
        f.flush()?;
    }
    std::fs::rename(&tam, duong_dan)?;
    Ok(())
}

// ---------------- locals.bin ----------------

#[derive(Clone, Debug)]
pub enum Locals {
    /// Rise (con trỏ 8 byte) / TR2013 (4 byte): chuỗi đánh số
    So {
        lang: i32,
        khoa: Vec<i32>,
        chuoi: std::collections::BTreeMap<usize, String>,
        n: usize,
        rong: usize,
    },
    /// Shadow: mỗi mục "khóa giá_trị"
    Khoa {
        lang: i32,
        ds: Vec<(String, String)>,
    },
}

fn chuoi_tai(b: &[u8], p: usize) -> Option<&[u8]> {
    let j = b[p..].iter().position(|&c| c == 0)?;
    Some(&b[p..p + j])
}

pub fn doc_locals_so(b: &[u8], rong: usize) -> Result<Locals> {
    let lang = i32_at(b, 0)?;
    let so_khoa = i32_at(b, 4)?.max(0) as usize;
    let n = i32_at(b, 8)?.max(0) as usize;
    let mut o = 12;
    let mut khoa = Vec::with_capacity(so_khoa);
    for _ in 0..so_khoa {
        khoa.push(i32_at(b, o)?);
        o += 4;
    }
    let mut chuoi = std::collections::BTreeMap::new();
    for i in 0..n {
        let p = if rong == 8 {
            u64_at(b, o + 8 * i)? as i64
        } else {
            i32_at(b, o + 4 * i)? as i64
        };
        if p <= 0 || p as usize >= b.len() {
            continue;
        }
        if let Some(s) = chuoi_tai(b, p as usize) {
            chuoi.insert(i, String::from_utf8_lossy(s).into_owned());
        }
    }
    Ok(Locals::So {
        lang,
        khoa,
        chuoi,
        n,
        rong,
    })
}

pub fn doc_locals_khoa(b: &[u8]) -> Result<Locals> {
    let lang = i32_at(b, 0)?;
    let n = i32_at(b, 4)?.max(0) as usize;
    let mut ds = Vec::new();
    for i in 0..n {
        let p = u64_at(b, 8 + 8 * i)? as i64;
        if p <= 0 || p as usize >= b.len() {
            continue;
        }
        let Some(s) = chuoi_tai(b, p as usize) else {
            continue;
        };
        let Some(k) = s.iter().position(|&c| c == b' ') else {
            continue;
        };
        ds.push((
            String::from_utf8_lossy(&s[..k]).into_owned(),
            String::from_utf8_lossy(&s[k + 1..]).into_owned(),
        ));
    }
    Ok(Locals::Khoa { lang, ds })
}

impl Locals {
    /// (khóa, chữ) của mọi chuỗi.
    pub fn cac_chuoi(&self) -> Vec<(String, &str)> {
        match self {
            Locals::So { chuoi, .. } => chuoi
                .iter()
                .map(|(k, v)| (k.to_string(), v.as_str()))
                .collect(),
            Locals::Khoa { ds, .. } => ds.iter().map(|(k, v)| (k.clone(), v.as_str())).collect(),
        }
    }

    /// Ghi lại với chữ mới (hàm đổi: chữ gốc → chữ mới | None giữ nguyên).
    pub fn ghi(&self, doi: &dyn Fn(&str) -> Option<String>) -> Vec<u8> {
        match self {
            Locals::So {
                lang,
                khoa,
                chuoi,
                n,
                rong,
            } => {
                let n = (*n).max(chuoi.keys().next_back().map_or(0, |k| k + 1));
                let mut dau = Vec::new();
                dau.extend(lang.to_le_bytes());
                dau.extend((khoa.len() as i32).to_le_bytes());
                dau.extend((n as i32).to_le_bytes());
                for k in khoa {
                    dau.extend(k.to_le_bytes());
                }
                let bat_dau = dau.len() + rong * n;
                let mut con_tro = vec![0u8; rong * n];
                let mut than = Vec::new();
                for (i, s) in chuoi {
                    let p = (bat_dau + than.len()) as u64;
                    if *rong == 8 {
                        con_tro[8 * i..8 * i + 8].copy_from_slice(&p.to_le_bytes());
                    } else {
                        con_tro[4 * i..4 * i + 4].copy_from_slice(&(p as i32).to_le_bytes());
                    }
                    than.extend(doi(s).unwrap_or_else(|| s.clone()).into_bytes());
                    than.push(0);
                }
                dau.extend(con_tro);
                dau.extend(than);
                dau
            }
            Locals::Khoa { lang, ds } => {
                let n = ds.len();
                let mut dau = Vec::new();
                dau.extend(lang.to_le_bytes());
                dau.extend((n as i32).to_le_bytes());
                let bat_dau = dau.len() + 8 * n;
                let mut con_tro = Vec::new();
                let mut than = Vec::new();
                for (k, v) in ds {
                    con_tro.extend(((bat_dau + than.len()) as u64).to_le_bytes());
                    than.extend(k.as_bytes());
                    than.push(b' ');
                    than.extend(doi(v).unwrap_or_else(|| v.clone()).into_bytes());
                    than.push(0);
                }
                dau.extend(con_tro);
                dau.extend(than);
                dau
            }
        }
    }
}

// ---------------- chữ cho font game ----------------

fn la_latin1(c: char) -> bool {
    ('\u{C0}'..='\u{FF}').contains(&c)
}

/// Font game chỉ có chữ có dấu Latin-1: giữ chữ font có, chữ thiếu thì ghép chữ cái + dấu rời; Đ → D.
pub fn dau_ghep(s: &str) -> String {
    let mut ra = String::new();
    for ch in s.nfc() {
        if (ch as u32) < 0x80 || la_latin1(ch) {
            ra.push(ch);
            continue;
        }
        if ch == 'đ' || ch == 'Đ' {
            ra.push(if ch == 'đ' { 'd' } else { 'D' });
            continue;
        }
        let d: Vec<char> = ch.to_string().nfd().collect();
        let (goc, dau) = (d[0], &d[1..]);
        if dau.is_empty() {
            ra.push(ch);
            continue;
        }
        let mut tot: String = d.iter().collect();
        for i in 0..dau.len() {
            let c: String = [goc, dau[i]].iter().collect::<String>().nfc().collect();
            let mut it = c.chars();
            if let (Some(c1), None) = (it.next(), it.next()) {
                if la_latin1(c1) {
                    tot = std::iter::once(c1)
                        .chain(dau[..i].iter().copied())
                        .chain(dau[i + 1..].iter().copied())
                        .collect();
                    break;
                }
            }
        }
        ra.push_str(&tot);
    }
    ra
}

/// Tiếng Việt không dấu.
pub fn bo_dau(s: &str) -> String {
    s.replace('đ', "d")
        .replace('Đ', "D")
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bam_va_locals() {
        // giá trị lấy từ app cũ (Python)
        assert_eq!(bam_cdc("pcx64-w\\local\\locals.bin"), {
            let mut h: u32 = 0xFFFFFFFF;
            for c in "pcx64-w\\local\\locals.bin".bytes() {
                h ^= (c as u32) << 24;
                for _ in 0..8 {
                    h = if h & 0x80000000 != 0 {
                        (h << 1) ^ 0x04C11DB7
                    } else {
                        h << 1
                    };
                }
            }
            !h
        });
        let l = Locals::So {
            lang: 1,
            khoa: vec![5, 6],
            chuoi: [(0, "Hello".to_string()), (2, "World".to_string())].into(),
            n: 4,
            rong: 8,
        };
        let b = l.ghi(&|s| (s == "Hello").then(|| "Xin chào".to_string()));
        let Locals::So { chuoi, n, khoa, .. } = doc_locals_so(&b, 8).unwrap() else {
            panic!()
        };
        assert_eq!((n, khoa), (4, vec![5, 6]));
        assert_eq!(chuoi[&0], "Xin chào");
        assert_eq!(chuoi[&2], "World");
        let k = Locals::Khoa {
            lang: 3,
            ds: vec![("a".into(), "One".into()), ("b".into(), "Two".into())],
        };
        let Locals::Khoa { ds, .. } = doc_locals_khoa(&k.ghi(&|_| None)).unwrap() else {
            panic!()
        };
        assert_eq!(ds[1], ("b".to_string(), "Two".to_string()));
        let z = khoi_cdrm(&vec![7u8; 300_000]);
        let tam = std::env::temp_dir().join("vh_cdrm_thu.bin");
        std::fs::write(&tam, &z).unwrap();
        let mut f = File::open(&tam).unwrap();
        assert_eq!(
            doc_tu_file(&mut f, 0, 0, false).unwrap(),
            vec![7u8; 300_000]
        );
    }

    #[test]
    fn chu_dau() {
        assert_eq!(bo_dau("Đường về nhà"), "Duong ve nha");
        // ế: ê có trong Latin-1 → ê + dấu sắc rời
        assert_eq!(dau_ghep("ế"), "ê\u{301}");
        assert_eq!(dau_ghep("đá"), "dá");
        assert_eq!(dau_ghep("ụ"), "u\u{323}");
    }
}
