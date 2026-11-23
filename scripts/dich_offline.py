# Dich offline quickmt-en-vi (CTranslate2 int8, CPU). App Viet Hoa chay file nay bang Python rieng.
# Giao thuc: moi dong stdin = mang JSON cac cau tieng Anh; moi dong stdout = mang JSON ban dich.
import json, os, sys

sys.stdout.reconfigure(encoding="utf-8")
sys.stdin.reconfigure(encoding="utf-8")
import ctranslate2
import sentencepiece as spm

d = sys.argv[1]
tr = ctranslate2.Translator(d, device="cpu", compute_type="int8",
                            intra_threads=min(6, os.cpu_count() or 4))
src = spm.SentencePieceProcessor(model_file=os.path.join(d, "src.spm.model"))
tgt = spm.SentencePieceProcessor(model_file=os.path.join(d, "tgt.spm.model"))
print("SAN_SANG", flush=True)
for dong in sys.stdin:
    try:
        cau = json.loads(dong.lstrip("﻿"))
        tok = src.encode(cau, out_type=str)
        kq = tr.translate_batch(tok, beam_size=2, max_decoding_length=256)
        ra = [tgt.decode(k.hypotheses[0]) for k in kq]
    except Exception as e:  # tra ve loi de app chuyen sang Google
        ra = {"loi": str(e)}
    print(json.dumps(ra, ensure_ascii=False), flush=True)
