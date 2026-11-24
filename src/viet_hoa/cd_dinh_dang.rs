//! Định dạng file Crimson Desert (bản 2.04+), chuyển từ crimson_desert.py của app cũ.
//!
//!   nhóm (VD 0020) = 0.pamt (mục lục) + N.paz (dữ liệu)
//!   file trong .paz: có thể mã hóa ChaCha20 (khóa từ hashlittle(tên file, 0xC5EDE)) và nén LZ4 khối.
//!   .paloc = header 0x200 "paloc" + 1 khối LZ4 → lặp [8 byte][u32 dài][khóa][u32 dài][chữ UTF-8] → đuôi.
//! Chỉ ĐỌC file game; bản dịch được đóng gói ra ngoài cho DMM nạp.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

// ---------------- hashlittle + ChaCha20 ----------------

fn u32_le(d: &[u8], o: usize) -> Result<u32> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| anyhow!("file hỏng: đọc quá cuối (vị trí {o})"))
}

pub fn hashlittle(data: &[u8], initval: u32) -> u32 {
    let mut n = data.len();
    let mut a = 0xDEADBEEFu32.wrapping_add(n as u32).wrapping_add(initval);
    let (mut b, mut c) = (a, a);
    let mut o = 0;
    let w = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    while n > 12 {
        a = a.wrapping_add(w(o));
        b = b.wrapping_add(w(o + 4));
        c = c.wrapping_add(w(o + 8));
        a = a.wrapping_sub(c);
        a ^= c.rotate_left(4);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= a.rotate_left(6);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= b.rotate_left(8);
        b = b.wrapping_add(a);
        a = a.wrapping_sub(c);
        a ^= c.rotate_left(16);
        c = c.wrapping_add(b);
        b = b.wrapping_sub(a);
        b ^= a.rotate_left(19);
        a = a.wrapping_add(c);
        c = c.wrapping_sub(b);
        c ^= b.rotate_left(4);
        b = b.wrapping_add(a);
        o += 12;
        n -= 12;
    }
    if n == 0 {
        return c;
    }
    let mut duoi = [0u8; 12];
    duoi[..n].copy_from_slice(&data[o..]);
    let t = |i: usize| u32::from_le_bytes(duoi[i * 4..i * 4 + 4].try_into().unwrap());
    let (w0, w1, w2) = (t(0), t(1), t(2));
    let m = u32::MAX;
    if n >= 9 {
        c = c.wrapping_add(w2 & (m >> (8 * (12 - n))));
    }
    if n >= 8 {
        b = b.wrapping_add(w1);
    } else if n >= 5 {
        b = b.wrapping_add(w1 & (m >> (8 * (8 - n))));
    }
    if n >= 4 {
        a = a.wrapping_add(w0);
    } else {
        a = a.wrapping_add(w0 & (m >> (8 * (4 - n))));
    }
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(14));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(11));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(25));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(16));
    a ^= c;
    a = a.wrapping_sub(c.rotate_left(4));
    b ^= a;
    b = b.wrapping_sub(a.rotate_left(14));
    c ^= b;
    c = c.wrapping_sub(b.rotate_left(24));
    c
}

/// Khóa (8 từ) và IV (4 từ: bộ đếm + nonce) theo tên file (chỉ lấy tên, chữ thường).
fn khoa_iv(ten_file: &str) -> ([u32; 8], [u32; 4]) {
    let ten = ten_file
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(ten_file)
        .to_lowercase();
    let seed = hashlittle(ten.as_bytes(), 0x000C_5EDE);
    let goc = seed ^ 0x6061_6263;
    let d = [
        0, 0x0A0A0A0A, 0x0C0C0C0C, 0x06060606, 0x0E0E0E0E, 0x0A0A0A0A, 0x06060606, 0x02020202,
    ];
    (d.map(|x| goc ^ x), [seed; 4])
}

/// ChaCha20 (đối xứng: mã hóa = giải mã).
pub fn chacha20(du_lieu: &[u8], ten_file: &str) -> Vec<u8> {
    let (k, iv) = khoa_iv(ten_file);
    let mut ra = du_lieu.to_vec();
    for (khoi, doan) in ra.chunks_mut(64).enumerate() {
        let mut s = [0u32; 16];
        s[..4].copy_from_slice(&[0x61707865, 0x3320646E, 0x79622D32, 0x6B206574]);
        s[4..12].copy_from_slice(&k);
        s[12] = iv[0].wrapping_add(khoi as u32);
        s[13..].copy_from_slice(&iv[1..]);
        let mut x = s;
        for _ in 0..10 {
            for [a, b, c, d] in [
                [0, 4, 8, 12],
                [1, 5, 9, 13],
                [2, 6, 10, 14],
                [3, 7, 11, 15],
                [0, 5, 10, 15],
                [1, 6, 11, 12],
                [2, 7, 8, 13],
                [3, 4, 9, 14],
            ] {
                x[a] = x[a].wrapping_add(x[b]);
                x[d] = (x[d] ^ x[a]).rotate_left(16);
                x[c] = x[c].wrapping_add(x[d]);
                x[b] = (x[b] ^ x[c]).rotate_left(12);
                x[a] = x[a].wrapping_add(x[b]);
                x[d] = (x[d] ^ x[a]).rotate_left(8);
                x[c] = x[c].wrapping_add(x[d]);
                x[b] = (x[b] ^ x[c]).rotate_left(7);
            }
        }
        let mut dong = [0u8; 64];
        for i in 0..16 {
            dong[i * 4..i * 4 + 4].copy_from_slice(&x[i].wrapping_add(s[i]).to_le_bytes());
        }
        for (p, q) in doan.iter_mut().zip(dong) {
            *p ^= q;
        }
    }
    ra
}

// ---------------- LZ4 khối ----------------

pub fn lz4_giai_nen(src: &[u8], co_goc: usize) -> Result<Vec<u8>> {
    let d = lz4_flex::block::decompress(src, co_goc).map_err(|e| anyhow!("LZ4: {e}"))?;
    if d.len() != co_goc {
        bail!("LZ4: sai kích thước sau giải nén");
    }
    Ok(d)
}

pub fn lz4_nen(d: &[u8]) -> Vec<u8> {
    lz4_flex::block::compress(d)
}

// ---------------- PAMT ----------------

#[derive(Clone, Debug)]
pub struct TepPamt {
    pub ten: String,
    pub thu_muc: String,
    pub duoi: String,
    pub vt: u32,
    pub nen: u32,
    pub goc: u32,
    pub paz: u32,
    pub kieu_nen: u8,
    pub ma_hoa: u8,
}

fn varint(d: &[u8], mut o: usize) -> Result<(u32, usize)> {
    let (mut v, mut s) = (0u32, 0);
    loop {
        let b = *d.get(o).ok_or_else(|| anyhow!("pamt hỏng"))?;
        o += 1;
        if s < 32 {
            v |= ((b & 0x7F) as u32) << s;
        }
        s += 7;
        if b < 0x80 {
            return Ok((v, o));
        }
    }
}

/// Cây tên (nút → (cha, mảnh tên)); trả (cây, vị trí sau cây).
type Cay = HashMap<u32, (Option<u32>, String)>;

fn cay(d: &[u8], mut o: usize, co: usize) -> Result<(Cay, usize)> {
    let dau = o;
    let mut nut = HashMap::new();
    while o < dau + co {
        let vt = (o - dau) as u32;
        let (lui, o2) = varint(d, o)?;
        let dl = *d.get(o2).ok_or_else(|| anyhow!("pamt hỏng"))? as usize;
        let manh = d
            .get(o2 + 1..o2 + 1 + dl)
            .ok_or_else(|| anyhow!("pamt hỏng"))?;
        let cha = (lui != 0).then(|| vt.wrapping_sub(lui));
        nut.insert(vt, (cha, String::from_utf8_lossy(manh).into_owned()));
        o = o2 + 1 + dl;
    }
    Ok((nut, o))
}

fn duong(nut: &Cay, mut r: Option<u32>) -> String {
    let mut ds = Vec::new();
    while let Some(k) = r {
        let Some((cha, t)) = nut.get(&k) else { break };
        ds.push(t.as_str());
        r = *cha;
        if ds.len() > 512 {
            break;
        }
    }
    ds.reverse();
    ds.concat()
}

/// Đọc mục lục 0.pamt: mọi file kèm thư mục, đuôi, kiểu nén / mã hóa.
pub fn doc_pamt(duong_dan: &Path) -> Result<Vec<TepPamt>> {
    let d = std::fs::read(duong_dan).with_context(|| format!("đọc {}", duong_dan.display()))?;
    let mut o = 4;
    let so_paz = u32_le(&d, o)? as usize;
    o += 12;
    for i in 0..so_paz {
        o += 8 + if i + 1 < so_paz { 4 } else { 0 };
    }
    let co = u32_le(&d, o)? as usize;
    let (nut_tm, o2) = cay(&d, o + 4, co)?;
    let co = u32_le(&d, o2)? as usize;
    let (nut, mut o) = cay(&d, o2 + 4, co)?;
    let so_tm = u32_le(&d, o)? as usize;
    o += 8;
    let mut cac_tm = Vec::with_capacity(so_tm);
    for i in 0..so_tm {
        let b = o + 16 * i;
        cac_tm.push([u32_le(&d, b)?, u32_le(&d, b + 4)?, u32_le(&d, b + 8)?]);
    }
    o += so_tm * 16;
    let mut ds = Vec::new();
    let mut so_duoi = Vec::new();
    while o + 20 <= d.len() {
        let ten = u32_le(&d, o)?;
        if !nut.contains_key(&ten) {
            break;
        }
        let co_bit = u32_le(&d, o + 16)?;
        ds.push(TepPamt {
            ten: duong(&nut, Some(ten)),
            thu_muc: String::new(),
            duoi: String::new(),
            vt: u32_le(&d, o + 4)?,
            nen: u32_le(&d, o + 8)?,
            goc: u32_le(&d, o + 12)?,
            paz: co_bit & 0xFFFF,
            kieu_nen: ((co_bit >> 16) & 0x0F) as u8,
            ma_hoa: ((co_bit >> 20) & 0x0F) as u8,
        });
        so_duoi.push((co_bit >> 24) as usize);
        o += 20;
    }
    let duoi: Vec<String> = if o + 4 <= d.len() {
        d[o + 4..]
            .split(|&b| b == 0)
            .map(|x| String::from_utf8_lossy(x).into_owned())
            .collect()
    } else {
        vec![String::new()]
    };
    for [vt, dau, so] in cac_tm {
        if so > 0 && nut_tm.contains_key(&vt) {
            let tm = duong(&nut_tm, Some(vt));
            let (dau, so) = (dau as usize, so as usize);
            for f in ds.iter_mut().skip(dau).take(so) {
                f.thu_muc = tm.clone();
            }
        }
    }
    for (f, s) in ds.iter_mut().zip(so_duoi) {
        f.duoi = duoi.get(s).cloned().unwrap_or_default();
    }
    Ok(ds)
}

fn doc_tho(thu_muc_nhom: &Path, f: &TepPamt) -> Result<Vec<u8>> {
    let mut fh = File::open(thu_muc_nhom.join(format!("{}.paz", f.paz)))?;
    fh.seek(SeekFrom::Start(f.vt as u64))?;
    let mut b = vec![0; f.nen as usize];
    fh.read_exact(&mut b)?;
    Ok(b)
}

/// Đọc 1 file trong nhóm (giải mã theo tên nếu mã hóa, giải nén LZ4 nếu nén).
pub fn doc_tep(thu_muc_nhom: &Path, f: &TepPamt) -> Result<Vec<u8>> {
    let mut b = doc_tho(thu_muc_nhom, f)?;
    if f.ma_hoa != 0 {
        b = chacha20(&b, &format!("{}{}", f.ten, f.duoi));
    }
    if f.kieu_nen == 2 {
        b = lz4_giai_nen(&b, f.goc as usize)?;
    }
    Ok(b)
}

// ---------------- PALOC ----------------

pub struct Paloc {
    pub header: Vec<u8>,
    /// (8 byte đầu, khóa, chữ)
    pub ds: Vec<([u8; 8], String, String)>,
    pub duoi: Vec<u8>,
}

/// header 0x200 "paloc" + khối LZ4 → (header, nội dung)
pub fn mo_boc(ban_giai_ma: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    if !ban_giai_ma.starts_with(b"paloc") || ban_giai_ma.len() < 0x200 {
        bail!("không phải file paloc (sai khóa giải mã?)");
    }
    let dai_khoi = u32_le(ban_giai_ma, 9)? as usize;
    let dai_goc = u32_le(ban_giai_ma, 13)? as usize;
    let khoi = ban_giai_ma
        .get(0x200..0x200 + dai_khoi)
        .ok_or_else(|| anyhow!("paloc hỏng"))?;
    Ok((ban_giai_ma[..0x200].to_vec(), lz4_giai_nen(khoi, dai_goc)?))
}

pub fn boc(header: &[u8], noi_dung: &[u8]) -> Vec<u8> {
    let khoi = lz4_nen(noi_dung);
    let mut h = header[..0x200].to_vec();
    h[9..13].copy_from_slice(&(khoi.len() as u32).to_le_bytes());
    h[13..17].copy_from_slice(&(noi_dung.len() as u32).to_le_bytes());
    h.extend(khoi);
    h
}

pub fn tach(nd: &[u8]) -> Result<Paloc> {
    let mut o = 0;
    let mut ds = Vec::new();
    while o + 16 <= nd.len() {
        if u32_le(nd, o + 8)? as usize > nd.len() {
            break;
        }
        let dau: [u8; 8] = nd[o..o + 8].try_into().unwrap();
        o += 8;
        let mut chuoi = || -> Result<String> {
            let n = u32_le(nd, o)? as usize;
            let b = nd
                .get(o + 4..o + 4 + n)
                .ok_or_else(|| anyhow!("paloc hỏng"))?;
            o += 4 + n;
            Ok(String::from_utf8(b.to_vec())?)
        };
        let k = chuoi()?;
        let v = chuoi()?;
        ds.push((dau, k, v));
    }
    Ok(Paloc {
        header: Vec::new(),
        ds,
        duoi: nd[o..].to_vec(),
    })
}

pub fn ghep(ds: &[([u8; 8], String, String)], duoi: &[u8]) -> Vec<u8> {
    let mut ra = Vec::new();
    for (dau, k, v) in ds {
        ra.extend_from_slice(dau);
        for s in [k, v] {
            ra.extend_from_slice(&(s.len() as u32).to_le_bytes());
            ra.extend_from_slice(s.as_bytes());
        }
    }
    ra.extend_from_slice(duoi);
    ra
}

/// Đọc mọi .paloc trong nhóm (VD 0020) → {tên file: Paloc}.
pub fn doc_nhom(thu_muc: &Path) -> Result<BTreeMap<String, Paloc>> {
    let mut kq = BTreeMap::new();
    for f in doc_pamt(&thu_muc.join("0.pamt"))? {
        let b = doc_tho(thu_muc, &f)?;
        let (header, nd) = mo_boc(&chacha20(&b, &format!("{}.paloc", f.ten)))
            .with_context(|| format!("{}.paloc", f.ten))?;
        let mut p = tach(&nd).with_context(|| format!("{}.paloc", f.ten))?;
        p.header = header;
        kq.insert(f.ten, p);
    }
    Ok(kq)
}

/// Tạo .paloc đã dịch (đã giải mã, bọc LZ4 như game – trình nạp mod tự xử lý).
/// `dich`: {khóa: chữ Việt}. Trả (bytes, số câu đã thay).
pub fn dong_goi_paloc(p: &Paloc, dich: &HashMap<String, String>) -> (Vec<u8>, usize) {
    let mut so = 0;
    let ds: Vec<_> =
        p.ds.iter()
            .map(|(dau, k, v)| match dich.get(k) {
                Some(vi) if vi != v => {
                    so += 1;
                    (*dau, k.clone(), vi.clone())
                }
                _ => (*dau, k.clone(), v.clone()),
            })
            .collect();
    (boc(&p.header, &ghep(&ds, &p.duoi)), so)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_va_chacha() {
        // giá trị chuẩn của lookup3 hashlittle
        assert_eq!(hashlittle(b"", 0), 0xdeadbeef);
        assert_eq!(hashlittle(b"Four score and seven years ago", 0), 0x17770551);
        assert_eq!(hashlittle(b"Four score and seven years ago", 1), 0xcd628161);
        let d: Vec<u8> = (0..300u32).map(|i| (i * 7) as u8).collect();
        let m = chacha20(&d, "ui.paloc");
        assert_ne!(m, d);
        assert_eq!(chacha20(&m, "UI.paloc"), d);
    }

    #[test]
    fn paloc_khu_hoi() {
        let ds = vec![
            (
                [1u8; 8],
                "k1".to_string(),
                "Hello <br/> {Param1}".to_string(),
            ),
            ([2u8; 8], "k2".to_string(), "Xin chào".to_string()),
        ];
        let nd = ghep(&ds, &[0, 0, 0, 0]);
        let p = tach(&nd).unwrap();
        assert_eq!(p.ds, ds);
        assert_eq!(p.duoi, vec![0, 0, 0, 0]);
        let mut h = vec![0u8; 0x200];
        h[..5].copy_from_slice(b"paloc");
        let b = boc(&h, &nd);
        let (h2, nd2) = mo_boc(&b).unwrap();
        assert_eq!((h2, nd2), (b[..0x200].to_vec(), nd));
    }

    /// Đọc nhóm chữ thật của game (chạy riêng: CD=<thư mục game> cargo test -- --ignored doc_that)
    #[test]
    #[ignore]
    fn doc_that() {
        let g = std::path::PathBuf::from(std::env::var("CD").unwrap());
        let nhom = doc_nhom(&g.join("0020")).unwrap();
        let tong: usize = nhom.values().map(|p| p.ds.len()).sum();
        println!("{} file, {tong} câu", nhom.len());
        for (t, p) in nhom.iter().take(3) {
            println!(
                "{t}: {:?}",
                p.ds.iter().take(2).map(|x| &x.2).collect::<Vec<_>>()
            );
        }
        // khứ hồi: giải nén → ghép lại → giống hệt
        for (t, p) in &nhom {
            let nd = ghep(&p.ds, &p.duoi);
            let b = boc(&p.header, &nd);
            let (_, nd2) = mo_boc(&b).unwrap();
            assert_eq!(nd, nd2, "{t}");
        }
        let ds = doc_pamt(&g.join("0008").join("0.pamt")).unwrap();
        let f = ds
            .iter()
            .find(|f| f.ten == "inventory" && f.duoi == ".staticinfobody")
            .expect("inventory");
        println!("inventory: {f:?}");
        let b = doc_tep(&g.join("0008"), f).unwrap();
        assert_eq!(b.len(), f.goc as usize);
    }
}
