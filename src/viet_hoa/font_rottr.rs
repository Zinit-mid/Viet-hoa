//! Font tiếng Việt cho Rise of the Tomb Raider – chuyển từ font_rottr.py.
//!  1. Đọc các "bộ sưu tập" (.drm v22) trong gói .tiger → tìm tài nguyên GFX chứa font Latin và các bộ
//!     sưu tập trỏ tới nó (kết quả dò lưu ở rottr/font_rottr.json, dùng chung định dạng với app cũ).
//!  2. Thêm chữ Việt vào font (font_viet).
//!  3. Ghi vào gói mod: tài nguyên mới (khối CDRM + dấu "NEXT") trong gói chung, bộ sưu tập trỏ sang đó.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};

use super::font_viet;
use super::tiger::{self, Muc, MAGIC_CDRM, MAGIC_TIGER};

const PHIEN_BAN_DO: i64 = 3;
const LOAI_BO: [u8; 9] = [1, 2, 5, 6, 9, 10, 12, 13, 15];

#[derive(Serialize, Deserialize, Clone)]
struct FontTim {
    /// (gói, phần, vị trí, cỡ gói, giải nén)
    loc: [i64; 5],
    p: String,
    kich: i64,
    md5: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct BstTim {
    p: String,
    bam: u64,
    locale: u64,
    phan: i32,
    vt: u32,
    co: u32,
    idx: Vec<usize>,
}

#[derive(Serialize, Deserialize)]
struct KetQuaDo {
    phien_ban: i64,
    chu_ky: String,
    font: Vec<FontTim>,
    bst: Vec<BstTim>,
}

pub struct GoiGame {
    pub p: PathBuf,
    pub ten: String,
    pub id: i32,
    pub uu: (i64, i64, i64, i64, usize),
    pub ds: Vec<Muc>,
}

fn file_do() -> PathBuf {
    crate::duong_dan::viet_hoa_game()
        .join("rottr")
        .join("font_rottr.json")
}

/// Các gói gốc của game (không gồm gói mod) kèm độ ưu tiên theo .nfo.
pub fn cac_goi(thu_muc: &Path) -> Vec<GoiGame> {
    let meta = super::tomb_raider::doc_meta(thu_muc);
    let mut ten: Vec<String> = std::fs::read_dir(thu_muc)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    ten.sort();
    let mut goi = Vec::new();
    for t in ten {
        if !t.ends_with(".000.tiger") || !t.starts_with("bigfile") || t.starts_with("bigfile._mod.")
        {
            continue;
        }
        let p = thu_muc.join(&t);
        let Ok((dau, ds)) = tiger::doc_tiger(&p) else {
            continue;
        };
        let m = meta.get(&(dau.id as i64));
        let lay = |k: &str, md: i64| m.and_then(|m| m.lay(k)).unwrap_or(md);
        let uu = (
            lay("version", 0),
            -lay("required", 1),
            lay("package", 0),
            lay("chunk", 0),
            super::tomb_raider::thu_tu_ngon_ngu(&t),
        );
        goi.push(GoiGame {
            p,
            ten: t,
            id: dau.id,
            uu,
            ds,
        });
    }
    goi
}

fn chu_ky(goi: &[GoiGame]) -> String {
    let mut s = Vec::new();
    for g in goi {
        let (co, luc) = std::fs::metadata(&g.p)
            .map(|m| {
                let luc = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs());
                (m.len(), luc)
            })
            .unwrap_or((0, 0));
        s.extend(format!("{}|{co}|{luc}", g.ten).into_bytes());
    }
    format!("{:x}", md5::compute(&s))
}

/// Đọc dữ liệu trong các gói, giữ file mở + khối đang giải nén dở.
#[derive(Default)]
struct Doc {
    f: HashMap<PathBuf, File>,
    nho: Option<((PathBuf, u64), Vec<u8>, Vec<(usize, usize)>, usize, u64)>,
}

impl Doc {
    fn mo(&mut self, p: &Path) -> Result<&mut File> {
        if !self.f.contains_key(p) {
            if self.f.len() > 40 {
                self.f.clear();
            }
            self.f.insert(p.to_path_buf(), File::open(p)?);
        }
        Ok(self.f.get_mut(p).unwrap())
    }

    /// `dai` byte từ vị trí `tu` (sau giải nén) của khối ở vt – giải nén dần, nhớ trạng thái.
    fn doan(&mut self, p: &Path, vt: u64, tu: usize, dai: usize) -> Result<Vec<u8>> {
        let can = tu + dai;
        let k = (p.to_path_buf(), vt);
        if self.nho.as_ref().map(|x| &x.0) != Some(&k) {
            let f = self.mo(p)?;
            f.seek(SeekFrom::Start(vt))?;
            let mut dau = [0u8; 16];
            let n = f.read(&mut dau)?;
            if n < 16 || u32::from_le_bytes(dau[..4].try_into().unwrap()) != MAGIC_CDRM {
                f.seek(SeekFrom::Start(vt + tu as u64))?;
                let mut b = vec![0u8; dai];
                let n = f.read(&mut b)?;
                b.truncate(n);
                return Ok(b);
            }
            let so = i32::from_le_bytes(dau[8..12].try_into().unwrap());
            if !(1..100_000).contains(&so) {
                bail!("khối nén hỏng");
            }
            let mut bang = vec![0u8; 8 * so as usize];
            f.read_exact(&mut bang)?;
            let khuc = (0..so as usize)
                .map(|i| {
                    let g = u32::from_le_bytes(bang[8 * i..8 * i + 4].try_into().unwrap()) as usize
                        >> 8;
                    let n =
                        u32::from_le_bytes(bang[8 * i + 4..8 * i + 8].try_into().unwrap()) as usize;
                    (g, n)
                })
                .collect();
            let q = (vt + 16 + 8 * so as u64 + 0xF) & !0xF;
            self.nho = Some((k, Vec::new(), khuc, 0, q));
        }
        let (_, mut du, khuc, mut i, mut q) = self.nho.take().unwrap();
        let f = self.mo(p)?;
        while du.len() < can && i < khuc.len() {
            let (goc, nen) = khuc[i];
            f.seek(SeekFrom::Start(q))?;
            let mut b = vec![0u8; nen];
            f.read_exact(&mut b)?;
            if nen == goc {
                du.extend(b);
            } else {
                flate2::read::ZlibDecoder::new(&b[..]).read_to_end(&mut du)?;
            }
            q = (q + nen as u64 + 0xF) & !0xF;
            i += 1;
        }
        let ra = du
            .get(tu.min(du.len())..can.min(du.len()))
            .unwrap_or(&[])
            .to_vec();
        self.nho = Some(((p.to_path_buf(), vt), du, khuc, i, q));
        Ok(ra)
    }

    /// Giải nén khối ở vt (CDRM hoặc thô). can: số byte cần (dừng sớm). toi_da: cỡ thô.
    fn khoi(&mut self, p: &Path, vt: u64, can: Option<usize>, toi_da: usize) -> Result<Vec<u8>> {
        let f = self.mo(p)?;
        f.seek(SeekFrom::Start(vt))?;
        let mut dau = [0u8; 16];
        let n = f.read(&mut dau)?;
        if n < 16 || u32::from_le_bytes(dau[..4].try_into().unwrap()) != MAGIC_CDRM {
            f.seek(SeekFrom::Start(vt))?;
            let mut b = vec![0u8; toi_da];
            let n = f.read(&mut b)?;
            b.truncate(n);
            return Ok(b);
        }
        let so = i32::from_le_bytes(dau[8..12].try_into().unwrap());
        if !(1..100_000).contains(&so) {
            bail!("khối nén hỏng");
        }
        let mut bang = vec![0u8; 8 * so as usize];
        f.read_exact(&mut bang)?;
        let mut q = (vt + 16 + 8 * so as u64 + 0xF) & !0xF;
        let mut ra = Vec::new();
        for i in 0..so as usize {
            let goc = u32::from_le_bytes(bang[8 * i..8 * i + 4].try_into().unwrap()) as usize >> 8;
            let nen = u32::from_le_bytes(bang[8 * i + 4..8 * i + 8].try_into().unwrap()) as usize;
            f.seek(SeekFrom::Start(q))?;
            let mut b = vec![0u8; nen];
            f.read_exact(&mut b)?;
            if nen == goc {
                ra.extend(b);
            } else {
                flate2::read::ZlibDecoder::new(&b[..]).read_to_end(&mut ra)?;
            }
            q = (q + nen as u64 + 0xF) & !0xF;
            if can.is_some_and(|c| ra.len() >= c) {
                break;
            }
        }
        Ok(ra)
    }
}

struct Drm {
    /// (thân, loại, ref)
    ids: Vec<(u32, u8, u32)>,
    locs: Vec<[i64; 5]>,
    vt_id: usize,
    vt_loc: usize,
}

fn doc_drm(b: &[u8]) -> Option<Drm> {
    if b.len() < 32 {
        return None;
    }
    let i = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let (ver, inc, dep, n) = (i(0), i(4), i(8), i(24));
    if ver != 22 || !(0..200_000).contains(&n) || inc < 0 || dep < 0 {
        return None;
    }
    let n = n as usize;
    let vt_id = 32;
    let vt_loc = vt_id + 20 * n + dep as usize + inc as usize;
    if vt_loc + 24 * n > b.len() {
        return None;
    }
    let mut ids = Vec::with_capacity(n);
    let mut locs = Vec::with_capacity(n);
    for k in 0..n {
        let o = vt_id + 20 * k;
        ids.push((u(o), b[o + 4], u(o + 8) >> 8));
        let o = vt_loc + 24 * k;
        let h = |o: usize| i16::from_le_bytes([b[o], b[o + 1]]) as i64;
        locs.push([
            h(o + 10),
            h(o + 8),
            u(o + 12) as i64,
            u(o + 16) as i64,
            u(o + 20) as i64,
        ]);
    }
    Some(Drm {
        ids,
        locs,
        vt_id,
        vt_loc,
    })
}

fn tim_gfx(du: &[u8]) -> Option<usize> {
    let het = du.len().saturating_sub(3).min(96);
    (0..het).find(|&k| {
        matches!(&du[k..k + 3], b"GFX" | b"CFX" | b"FWS" | b"CWS") && (6..=20).contains(&du[k + 3])
    })
}

fn do_font(thu_muc: &Path, bao: &dyn Fn(&str), dung: &AtomicBool) -> Result<KetQuaDo> {
    let goi = cac_goi(thu_muc);
    if goi.is_empty() {
        bail!("không đọc được gói .tiger nào của game");
    }
    let ck = chu_ky(&goi);
    if let Ok(b) = std::fs::read(file_do()) {
        if let Ok(cu) = serde_json::from_slice::<KetQuaDo>(&b) {
            if cu.chu_ky == ck && cu.phien_ban == PHIEN_BAN_DO && !cu.font.is_empty() {
                return Ok(cu);
            }
        }
    }
    // file có hiệu lực: (băm, locale) → gói ưu tiên cao nhất (giữ thứ tự gặp đầu tiên)
    let mut thu_tu: Vec<usize> = (0..goi.len()).collect();
    thu_tu.sort_by_key(|&i| goi[i].uu);
    let mut chi_so: HashMap<(u64, u64), usize> = HashMap::new();
    let mut hl: Vec<(usize, usize)> = Vec::new();
    for gi in thu_tu {
        for (mi, m) in goi[gi].ds.iter().enumerate() {
            match chi_so.get(&(m.bam, m.locale)) {
                Some(&k) => hl[k] = (gi, mi),
                None => {
                    chi_so.insert((m.bam, m.locale), hl.len());
                    hl.push((gi, mi));
                }
            }
        }
    }
    let mut theo_id: HashMap<i32, Vec<usize>> = HashMap::new();
    for (i, g) in goi.iter().enumerate() {
        theo_id.entry(g.id).or_default().push(i);
    }
    for v in theo_id.values_mut() {
        v.sort_by_key(|&i| goi[i].ten.matches('_').count());
    }
    let mut doc = Doc::default();
    let mut bst: Vec<(usize, usize, Drm)> = Vec::new();
    let tong = hl.len();
    for (i, &(gi, mi)) in hl.iter().enumerate() {
        if dung.load(Ordering::SeqCst) {
            bail!("đã dừng");
        }
        if i % 2000 == 0 {
            bao(&format!(
                "Đang dò font chữ (lần đầu): đọc danh mục {}%...",
                i * 100 / tong.max(1)
            ));
        }
        let m = &goi[gi].ds[mi];
        if m.co < 32 || m.co > (32 << 20) {
            continue;
        }
        let p = tiger::duong_dan_phan(&goi[gi].p, m.phan);
        let Ok(dau) = doc.khoi(&p, m.vt as u64, Some(32), 32) else {
            continue;
        };
        if dau.len() < 32 || i32::from_le_bytes(dau[..4].try_into().unwrap()) != 22 {
            continue;
        }
        let Ok(du) = doc.khoi(&p, m.vt as u64, None, m.co as usize) else {
            continue;
        };
        if let Some(d) = doc_drm(&du) {
            bst.push((gi, mi, d));
        }
    }
    // tài nguyên ứng viên
    let mut ung: Vec<([i64; 5], i64)> = Vec::new();
    let mut da: std::collections::HashSet<[i64; 5]> = Default::default();
    for (_, _, d) in &bst {
        for (idn, loc) in d.ids.iter().zip(&d.locs) {
            let kich = idn.0 as i64 + idn.2 as i64;
            if LOAI_BO.contains(&idn.1) || !(2000 < kich && kich < (6 << 20)) {
                continue;
            }
            if da.insert(*loc) {
                ung.push((*loc, kich));
            }
        }
    }
    ung.sort_by_key(|x| x.0);
    let mut font = Vec::new();
    let mut moc = std::time::Instant::now();
    for (j, (loc, kich)) in ung.iter().enumerate() {
        if dung.load(Ordering::SeqCst) {
            bail!("đã dừng");
        }
        if moc.elapsed().as_secs_f32() > 1.5 {
            moc = std::time::Instant::now();
            bao(&format!(
                "Đang dò font chữ: tài nguyên {j}/{} ({}%)...",
                ung.len(),
                j * 100 / ung.len().max(1)
            ));
        }
        let [goi_id, phan, vt, _, _] = *loc;
        for &gi in theo_id
            .get(&(goi_id as i32))
            .map(Vec::as_slice)
            .unwrap_or(&[])
        {
            let p = tiger::duong_dan_phan(&goi[gi].p, phan as i32);
            if !p.is_file() {
                continue;
            }
            let Ok(du) = doc.doan(&p, vt as u64, 0, 120) else {
                continue;
            };
            let Some(k) = tim_gfx(&du) else { continue };
            if let Ok(toan) = doc.doan(&p, vt as u64, 0, *kich as usize) {
                if font_viet::co_font_latin(&toan[k.min(toan.len())..]) {
                    font.push(FontTim {
                        loc: *loc,
                        p: goi[gi].ten.clone(),
                        kich: *kich,
                        md5: format!("{:x}", md5::compute(&toan)),
                    });
                }
            }
            break;
        }
    }
    if font.is_empty() {
        bail!("không tìm thấy thư viện font chữ Latin của game");
    }
    let locs_font: std::collections::HashSet<[i64; 5]> = font.iter().map(|x| x.loc).collect();
    let mut dung_bst = Vec::new();
    for (gi, mi, d) in &bst {
        let idx: Vec<usize> = d
            .locs
            .iter()
            .enumerate()
            .filter(|(_, l)| locs_font.contains(*l))
            .map(|(i, _)| i)
            .collect();
        if !idx.is_empty() {
            let m = &goi[*gi].ds[*mi];
            dung_bst.push(BstTim {
                p: goi[*gi].ten.clone(),
                bam: m.bam,
                locale: m.locale,
                phan: m.phan,
                vt: m.vt,
                co: m.co,
                idx,
            });
        }
    }
    let kq = KetQuaDo {
        phien_ban: PHIEN_BAN_DO,
        chu_ky: ck,
        font,
        bst: dung_bst,
    };
    if let Some(cha) = file_do().parent() {
        let _ = std::fs::create_dir_all(cha);
    }
    crate::duong_dan::ghi_file(&file_do(), serde_json::to_string(&kq)?.as_bytes())?;
    Ok(kq)
}

/// Bộ sưu tập cần ghi lại: (băm, locale, nội dung gốc, drm) – tạo bản mới khi biết vị trí tài nguyên.
pub struct BstMoi {
    pub bam: u64,
    pub locale: u64,
    du: Vec<u8>,
    d: Drm,
}

pub struct FontMoi {
    /// khối CDRM của từng tài nguyên font mới
    pub tai_nguyen: Vec<Vec<u8>>,
    cac_loc: Vec<[i64; 5]>,
    kich_moi: Vec<usize>,
    pub bst: Vec<BstMoi>,
}

impl FontMoi {
    /// Bộ sưu tập mới trỏ sang tài nguyên font trong gói `goi_id` ở các vị trí `cac_vt`.
    pub fn tao_bst(&self, b: &BstMoi, goi_id: i32, cac_vt: &[u32]) -> Vec<u8> {
        let mut ra = b.du.clone();
        for (i, loc) in b.d.locs.iter().enumerate() {
            let Some(j) = self.cac_loc.iter().position(|l| l == loc) else {
                continue;
            };
            let r = b.d.ids[i].2 as usize;
            let o = b.d.vt_id + 20 * i;
            ra[o..o + 4].copy_from_slice(&((self.kich_moi[j] - r) as u32).to_le_bytes());
            let o = b.d.vt_loc + 24 * i + 8;
            ra[o..o + 2].copy_from_slice(&0i16.to_le_bytes());
            ra[o + 2..o + 4].copy_from_slice(&(goi_id as i16).to_le_bytes());
            ra[o + 4..o + 8].copy_from_slice(&cac_vt[j].to_le_bytes());
            ra[o + 8..o + 12].copy_from_slice(&(self.tai_nguyen[j].len() as u32).to_le_bytes());
            ra[o + 12..o + 16].copy_from_slice(&0u32.to_le_bytes());
        }
        ra
    }
}

/// Tạo dữ liệu font Việt để đưa vào gói mod.
pub fn chuan_bi(
    thu_muc: &Path,
    ttf: &[u8],
    bao: &dyn Fn(&str),
    dung: &AtomicBool,
) -> Result<FontMoi> {
    let kq = do_font(thu_muc, bao, dung)?;
    let goi: HashMap<String, GoiGame> = cac_goi(thu_muc)
        .into_iter()
        .map(|g| (g.ten.clone(), g))
        .collect();
    let mut doc = Doc::default();
    let mut moi: Vec<([i64; 5], Vec<u8>)> = Vec::new();
    for x in &kq.font {
        let g = goi
            .get(&x.p)
            .ok_or_else(|| anyhow!("game đã thay đổi – xóa kết quả dò và thử lại"))?;
        let [_, phan, vt, _, _] = x.loc;
        let p = tiger::duong_dan_phan(&g.p, phan as i32);
        let toan = doc.doan(&p, vt as u64, 0, x.kich as usize)?;
        let k = tim_gfx(&toan).ok_or_else(|| anyhow!("không thấy font trong tài nguyên"))?;
        let mut gfx_goc = &toan[k..];
        let l = u32::from_le_bytes(gfx_goc[4..8].try_into().unwrap()) as usize;
        let mut duoi: &[u8] = &[];
        if gfx_goc[0] == b'G' || gfx_goc[0] == b'F' {
            duoi = &gfx_goc[l.min(gfx_goc.len())..];
            gfx_goc = &gfx_goc[..l.min(gfx_goc.len())];
        }
        bao("Đang thêm chữ tiếng Việt vào font của game...");
        let (gfx_moi, so) = font_viet::them_tieng_viet(gfx_goc, ttf)?;
        if so == 0 {
            continue;
        }
        let mut truoc = toan[..k].to_vec();
        if k >= 4 && u32::from_le_bytes(truoc[k - 4..k].try_into().unwrap()) as usize == l {
            truoc[k - 4..k].copy_from_slice(&(gfx_moi.len() as u32).to_le_bytes());
        }
        truoc.extend(gfx_moi);
        truoc.extend(duoi);
        moi.push((x.loc, truoc));
    }
    if moi.is_empty() {
        bail!("font của game đã có đủ chữ tiếng Việt");
    }
    let cac_loc: Vec<[i64; 5]> = moi.iter().map(|x| x.0).collect();
    let kich_moi: Vec<usize> = moi.iter().map(|x| x.1.len()).collect();
    let tai_nguyen: Vec<Vec<u8>> = moi.iter().map(|x| tiger::khoi_cdrm(&x.1)).collect();
    let mut bst = Vec::new();
    for b in &kq.bst {
        let Some(g) = goi.get(&b.p) else { continue };
        let du = doc.khoi(
            &tiger::duong_dan_phan(&g.p, b.phan),
            b.vt as u64,
            None,
            b.co as usize,
        )?;
        let Some(d) = doc_drm(&du) else { continue };
        bst.push(BstMoi {
            bam: b.bam,
            locale: b.locale,
            du,
            d,
        });
    }
    Ok(FontMoi {
        tai_nguyen,
        cac_loc,
        kich_moi,
        bst,
    })
}

/// Gói chung của mod (v4): các khối tài nguyên (mỗi khối + dấu "NEXT") rồi các file.
/// cac_tep: (băm, locale, hàm(vị trí tài nguyên) → dữ liệu). Trả vị trí các tài nguyên.
pub fn tao_goi_chung(
    duong_dan: &Path,
    ma: i32,
    tai_nguyen: &[Vec<u8>],
    mut cac_tep: Vec<(u64, u64, Box<dyn Fn(&[u32]) -> Vec<u8> + '_>)>,
) -> Result<Vec<u32>> {
    cac_tep.sort_by_key(|x| (x.0, x.1));
    let mut dau = Vec::new();
    for v in [MAGIC_TIGER as i32, 4, 1, cac_tep.len() as i32, ma] {
        dau.extend(v.to_le_bytes());
    }
    let mut t = b"pcx64-w".to_vec();
    t.resize(0x20, 0);
    dau.extend(t);
    let vt = dau.len() + 24 * cac_tep.len();
    let mut than: Vec<u8> = Vec::new();
    let mut cac_vt: Vec<u32> = Vec::new();
    let mut dau_next: Vec<usize> = Vec::new();
    let dem = |than: &mut Vec<u8>| {
        let n = (16 - (vt + than.len()) % 16) % 16;
        than.resize(than.len() + n, 0);
    };
    for tn in tai_nguyen {
        let o = (vt + than.len() + 0xF) & !0xF;
        than.resize(o - vt, 0);
        if let Some(&dn) = dau_next.last() {
            let k = dn - vt + 4;
            than[k..k + 4].copy_from_slice(&((o - dn) as u32).to_le_bytes());
        }
        cac_vt.push(o as u32);
        than.extend(tn);
        dem(&mut than);
        dau_next.push(vt + than.len());
        than.extend(0x5458_454Eu32.to_le_bytes());
        than.extend(0u32.to_le_bytes());
        dem(&mut than);
    }
    let mut bang = Vec::new();
    for (bam, loc, ham) in &cac_tep {
        let du = ham(&cac_vt);
        let o = (vt + than.len() + 0xF) & !0xF;
        than.resize(o - vt, 0);
        bang.extend((*bam as u32).to_le_bytes());
        bang.extend((*loc as u32).to_le_bytes());
        bang.extend((du.len() as u32).to_le_bytes());
        bang.extend(0u32.to_le_bytes());
        bang.extend(0i16.to_le_bytes());
        bang.extend((ma as i16).to_le_bytes());
        bang.extend((o as u32).to_le_bytes());
        than.extend(du);
    }
    dau.extend(bang);
    dau.extend(than);
    crate::duong_dan::ghi_file(duong_dan, &dau)?;
    Ok(cac_vt)
}
