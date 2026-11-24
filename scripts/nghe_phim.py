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
    import yt_dlp

    def moc(d):
        if d.get("status") == "downloading":
            tong = d.get("total_bytes") or d.get("total_bytes_estimate") or 0
            if tong:
                bao("TAI", round(d.get("downloaded_bytes", 0) * 100 / tong, 1))
    tuy = {"outtmpl": os.path.join(cfg["thu_muc_tai"], "%(title).80s [%(id)s].%(ext)s"),
           "format": "bv*[height<=1080][ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*[height<=1080]+ba/b",
           "merge_output_format": "mp4", "ffmpeg_location": os.path.dirname(cfg["ffmpeg"]),
           "progress_hooks": [moc], "noplaylist": True, "quiet": True, "no_warnings": True,
           "windowsfilenames": True}
    for g, _, ts in os.walk(cfg.get("deno_tim") or "."):
        if "deno.exe" in ts:
            tuy["js_runtimes"] = {"deno": {"path": os.path.join(g, "deno.exe")}}
            break
    with yt_dlp.YoutubeDL(tuy) as y:
        tt = y.extract_info(nguon, download=True)
        p = y.prepare_filename(tt)
        if not os.path.isfile(p):
            p = os.path.splitext(p)[0] + ".mp4"
    bao("FILE", json.dumps({"duong_dan": p, "ten": tt.get("title", "")}, ensure_ascii=False))
    nguon = p
if cfg.get("chi_tai"):
    sys.exit(0)

from faster_whisper import WhisperModel
os.environ.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
m = None
if cfg.get("gpu", True):
    try:
        m = WhisperModel(cfg["mo_hinh"], device="cuda", compute_type="float16", download_root=cfg["mo_hinh_dir"])
        bao("THIET_BI cuda")
    except Exception as e:
        bao("KHONG_CUDA", str(e)[:300])
        if not cfg.get("cho_cpu"):
            sys.exit(3)
if m is None:
    m = WhisperModel(cfg.get("mo_hinh_cpu", "small"), device="cpu", compute_type="int8",
                     cpu_threads=max(2, (os.cpu_count() or 4) - 2), download_root=cfg["mo_hinh_dir"])
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
