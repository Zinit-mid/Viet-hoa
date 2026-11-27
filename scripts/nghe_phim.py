# Tai phim (yt-dlp) + nghe ra chu (faster-whisper, GPU) bang Python rieng cua app (cong_cu\python).
# Doc argv[1] = viec.json. In: TAI %, FILE {json}, THIET_BI, KHONG_CUDA, GIAI_MA, NGON_NGU, TIEN a b, HET.
import json, os, sys
cfg = json.load(open(sys.argv[1], encoding="utf-8"))
for d in cfg.get("dll", []):
    if os.path.isdir(d):
        try:
            os.add_dll_directory(d)
        except Exception:
            pass
        os.environ["PATH"] = d + os.pathsep + os.environ.get("PATH", "")


def bao(*a):
    print(*a, flush=True)


nguon = cfg["nguon"]
if cfg.get("tai"):
    import time
    import yt_dlp

    TRE = 45  # giay khong nhan them byte nao -> coi la ket, thu cach khac

    class Ket(Exception):
        pass

    def tao_moc(mo):
        def moc(d):
            if d.get("status") != "downloading":
                return
            n = d.get("downloaded_bytes", 0) or 0
            fc, fi = d.get("fragment_count"), d.get("fragment_index")
            if fc and fi is not None:
                pt = fi * 100.0 / fc
            else:
                tong = d.get("total_bytes") or d.get("total_bytes_estimate") or 0
                pt = n * 100.0 / tong if tong else None
            if n > mo["n"] or (fi or 0) > mo["fi"]:
                mo["n"], mo["fi"], mo["t"] = n, (fi or 0), time.time()
            elif time.time() - mo["t"] > TRE:
                raise Ket("tai bi ket " + str(TRE) + " giay")
            if pt is not None:
                bao("TAI", round(min(pt, 99.9), 1))
        return moc

    cach = [
        ("bv*[height<=1080][ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*[height<=1080]+ba/b", None),
        ("b[protocol=https][ext=mp4]/b[protocol=https]/b[ext=mp4]/b", None),
        ("b[protocol=https][ext=mp4]/b[protocol=https]/b", ["tv", "web_safari", "ios"]),
    ]
    deno = None
    for g, _, ts in os.walk(cfg.get("deno_tim") or "."):
        if "deno.exe" in ts:
            deno = os.path.join(g, "deno.exe")
            break
    tt = p = None
    loi_cuoi = ""
    for fmt, may in cach:
        bao("CACH", fmt[:30])
        mo = {"n": 0, "fi": 0, "t": time.time()}
        tuy = {"outtmpl": os.path.join(cfg["thu_muc_tai"], "%(title).80s [%(id)s].%(ext)s"),
               "format": fmt, "merge_output_format": "mp4", "ffmpeg_location": os.path.dirname(cfg["ffmpeg"]),
               "progress_hooks": [tao_moc(mo)], "noplaylist": True, "quiet": True, "no_warnings": True,
               "windowsfilenames": True, "socket_timeout": 20, "retries": 3, "fragment_retries": 3,
               "extractor_retries": 2}
        if may:
            tuy["extractor_args"] = {"youtube": {"player_client": may}}
        if deno:
            tuy["js_runtimes"] = {"deno": {"path": deno}}
        try:
            with yt_dlp.YoutubeDL(tuy) as y:
                tt = y.extract_info(nguon, download=True)
                p = y.prepare_filename(tt)
                if not os.path.isfile(p):
                    p = os.path.splitext(p)[0] + ".mp4"
            if os.path.isfile(p) and os.path.getsize(p) > 100000:
                break
            loi_cuoi = "file tai ve rong"
            p = None
        except Exception as e:
            loi_cuoi = str(e)[-300:]
            bao("LOI_TAI", loi_cuoi.replace("\n", " "))
            p = None
    if not p:
        sys.stderr.write("khong tai duoc phim: " + loi_cuoi + "\n")
        sys.exit(4)
    bao("TAI", 100)
    bao("FILE", json.dumps({"duong_dan": p, "ten": tt.get("title", "")}, ensure_ascii=False))
    nguon = p
if cfg.get("chi_tai"):
    sys.exit(0)

from faster_whisper import WhisperModel
os.environ.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
m = None


def nap(thiet_bi, loai, ten):
    # Da co mo hinh trong may thi nap thang, khong hoi mang (mang cham/chan HuggingFace lam treo vai phut)
    try:
        return WhisperModel(ten, device=thiet_bi, compute_type=loai, download_root=cfg["mo_hinh_dir"],
                            local_files_only=True, **({"cpu_threads": max(2, (os.cpu_count() or 4) - 2)} if thiet_bi == "cpu" else {}))
    except Exception:
        bao("TAI_MO_HINH")
        return WhisperModel(ten, device=thiet_bi, compute_type=loai, download_root=cfg["mo_hinh_dir"],
                            **({"cpu_threads": max(2, (os.cpu_count() or 4) - 2)} if thiet_bi == "cpu" else {}))


bao("NAP_MO_HINH")
if cfg.get("gpu", True):
    try:
        m = nap("cuda", "float16", cfg["mo_hinh"])
        bao("THIET_BI cuda")
    except Exception as e:
        bao("KHONG_CUDA", str(e)[:300])
        if not cfg.get("cho_cpu"):
            sys.exit(3)
if m is None:
    m = nap("cpu", "int8", cfg.get("mo_hinh_cpu", "small"))
    bao("THIET_BI cpu")
import subprocess, numpy as np
bao("GIAI_MA")
raw = subprocess.run([cfg["ffmpeg"], "-nostdin", "-v", "error", "-i", nguon, "-vn", "-ac", "1",
                      "-ar", "16000", "-f", "s16le", "-"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                     creationflags=0x08000000 if os.name == "nt" else 0).stdout
am = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
del raw
if not len(am):
    raise SystemExit("khong doc duoc am thanh trong file phim")
doan, tt = m.transcribe(am, language=cfg.get("ngon_ngu") or None, vad_filter=True,
                        word_timestamps=True, beam_size=5, condition_on_previous_text=False)
bao("NGON_NGU", tt.language, round(tt.duration, 1))
ra = []
for s in doan:
    ra.append({"a": s.start, "b": s.end, "chu": s.text,
               "tu": [[w.start, w.end, w.word] for w in (s.words or [])]})
    bao("TIEN", round(s.end, 1), round(tt.duration, 1))
with open(cfg["ra"], "w", encoding="utf-8") as f:
    json.dump({"ngon_ngu": tt.language, "giay": tt.duration, "doan": ra}, f, ensure_ascii=False)
bao("HET")
