# Việt Hóa – ghi chú dự án

App Rust viết lại app Python "Dịch Game" (repo cũ Zinit-mid/DichGame, mã tham khảo ở `tham_khao\DichGame`).
Chuyển hết chức năng app cũ nhưng gọn hơn và bỏ các nút thử. Cập nhật lần cuối: 24/11/2026.

## Tiến độ

| GĐ | Nội dung | Trạng thái |
|---|---|---|
| 1 | Dịch màn hình (OCR, khung dịch, F6–F12) | Xong (c876f55) |
| 2 | Lõi `ViecDich` dùng chung (Claude theo lô / offline, bộ nhớ dịch) | Xong (c876f55) |
| 3 | Thư viện game (Steam / Epic / GOG / thêm tay) + Dò file chữ 9 định dạng | Xong (12f183d) |
| 4 | Crimson Desert: chữ (paz / paloc), gói DMM, mod túi đồ | Xong (4450b13) |
| 5 | Rise / TR2013 / Shadow: chữ, phụ đề .mul, lời thoại TextEntry, font Việt (Rise), cài / gỡ | Xong, chưa commit |
| 6 | Lồng tiếng + Clone giọng (VieNeu, Wwise cho CD, FSB4 cho TR; cài giọng TR2013 vào patch2_ENGLISH) | **Đang làm:** lõi VieNeu xong (`giong_vieneu.rs` + `scripts\tao_giong.py`, đã đọc thật 2 câu trên GPU). Wwise xong (`wwise.rs`: WAV→.wem→zip DMM, đã mã hóa thật 1 câu). Lồng tiếng Rise/TR2013 xong (`TombRaider::long_tieng`, `phien_am.rs`, `scripts\long_tieng_tr.py`; thử 16 đoạn đầu Rise, MP3 đóng được FSB4; chưa thử trong game, TR2013 chưa chạy thật). Giao diện lồng tiếng TR có trong thẻ game (chưa xem bằng mắt). Lồng tiếng CD xong phần code (`cd_giong.rs`: kế hoạch A/B đọc đúng 64.813 file .wem, giọng mẫu bằng vgmstream đã thử với Kliff; thẻ giao diện CD chưa xem bằng mắt; chưa chạy trọn A/B vì rất lâu, tự sửa giới tính F0 đã port, khớp 119/119 người với app cũ; chưa có học phiên âm bằng Claude). Clone giọng xong (`clone_giong.rs` + trang `trang_clone_giong.rs`: cắt/bỏ lặng/lọc 70 Hz/chuẩn hóa, nghe thử bằng VieNeu, lưu kho `giong_rieng`, gán `phan_bo.json`; đã thử thật 1 mẫu + 1 câu; chưa thử nút bấm). Xong GĐ6 phần code |
| 7 | Phụ đề phim (Whisper) + giọng đọc | Xong, chưa commit: `phu_de_phim.rs` + trang `trang_phu_de_phim.rs` (yt-dlp, faster-whisper GPU, dịch Claude/offline, .srt, mở VLC); đã chạy thật 60 giây phim (nghe 5 s, ra .srt Việt). Giọng đọc (edge-tts) có từ GĐ1 trong Dịch màn hình. Chưa thử tải link YouTube thật |

**Việc còn lại:**
- **Claude:** đường dịch bằng Claude chưa gọi thật lần nào (tránh tốn tiền).
- **Kiểm tra trong game thật:**
  - Crimson Desert: gói DMM và mod túi đồ.
  - TR2013 và Shadow: bản cài.
- **Rise:** gói mới giống từng byte gói app cũ đã cài, nên coi như đã kiểm.

## Quyết định kiến trúc

**Thư mục và dữ liệu**
- **Mọi thứ nằm trong `D:\Việt hóa`:**
  - `cong_cu` chứa Rust, Build Tools, python, ffmpeg…
  - `du_lieu` chứa cấu hình, bộ nhớ dịch, mục lục.
  - Không ghi gì vào `%APPDATA%` hay ổ C.
- **Tìm thư mục gốc** (`duong_dan::goc()`), theo thứ tự:
  1. Thư mục có `Cargo.toml`.
  2. Thư mục chứa exe.
  3. Nếu cạnh exe không có `cong_cu` thì dùng `D:\Việt hóa`, hoặc biến `VIET_HOA_GOC`.
  - Biến `VIET_HOA_DU_LIEU` đổi riêng thư mục dữ liệu, dùng khi kiểm thử.
- **Dữ liệu tương thích app cũ:**
  - Giữ đúng định dạng: bộ nhớ dịch, `cau_hinh.json`, mục lục `.mul` / TextEntry, `font_rottr.json`.
  - Giữ đúng tên gói mod (`viethoa_dichgame`), để hai app dùng chung dữ liệu và gỡ được gói của nhau.
- **Khóa API** chỉ nằm trong `du_lieu\khoa_api.txt`, không bao giờ đưa vào code hay repo.

**Dịch màn hình**
- Chụp màn hình bằng DXGI Desktop Duplication. GDI là dự phòng, kèm thời gian chờ khi lỗi (`quan_ly_chup`).
- Lớp phủ là cửa sổ layered, xuyên chuột và loại khỏi ảnh chụp (`WDA_EXCLUDEFROMCAPTURE`).
- Cửa sổ app được ẩn khỏi ảnh chụp khi đang dịch.
- Mỗi vùng chỉ dịch câu mới nhất.
- Thứ tự tra bản dịch:
  1. Bản Việt hóa
  2. Bản Claude đã lưu
  3. Từ điển menu
  4. Offline (quickmt qua `scripts\dich_offline.py`)
  5. Google

**Lõi dịch và game**
- **`viet_hoa::viec_dich::ViecDich`** là lõi dùng chung cho mọi game:
  - Claude chạy 4 luồng; mỗi lô tối đa 40 câu / 6.000 ký tự; có giới hạn USD mỗi lần.
  - Dịch offline giữ nguyên thẻ.
  - Hàm kiểm thẻ cắm vào được (`ViecDich::the`); Tomb Raider dùng `the_tr`.
- **Mỗi game có công cụ riêng** trong `src/viet_hoa/`:
  - Crimson Desert: `cd_dinh_dang`, `crimson_desert`.
  - Tomb Raider: `tiger`, `mul`, `font_viet`, `font_rottr`, `tomb_raider`.
  - Mọi game khác: `do_file` (Dò file chữ).
- **Không sửa file gốc của game:**
  - Crimson Desert: xuất zip cho DMM.
  - Rise / Shadow: gói mod riêng; gỡ = xóa gói.
  - TR2013: buộc phải tạo lại `patch2`, nhưng giữ bản gốc `bigfile._orig.patch2.000.tiger`.
  - Dò file chữ: sao lưu trước khi ghi và luôn dịch từ bản gốc.
- **Giọng đã lồng của app cũ** (`<ổ>:\DichGame_LongTieng\RotTR|TR2013\mp3`) được cài kèm khi cài Rise, để không mất lồng tiếng.

**Giao diện**
- egui (`eframe` 0.31), tắt lưu trạng thái cửa sổ.
- Việc nặng chạy nền qua `chay_nen`, có thanh tiến trình và nút Dừng.
- Font giao diện không có ký tự ✓ → ✗; dùng chữ thường thay thế.

## Cách kiểm thử

**Lệnh chung**
- `cargo test --release` chạy test thường.
- Các test `#[ignore]` chạy với dữ liệu thật, qua biến môi trường. Ví dụ:
  - `CD=<thư mục game> cargo test -- --ignored doc_that`
  - `TR=<thư mục> LOAI=rise|tr2013|shadow cargo test -- --ignored cai_that` (thêm `CHI_NAP=1` để chỉ đọc, `GO=1` để thử gỡ)

**Cài vào game**
- **Không cài thử vào thư mục game thật.** Tạo thư mục giả trên cùng ổ bằng liên kết cứng (`New-Item -ItemType HardLink`).
- **Ngoại lệ:** file bị ghi đè trực tiếp (vd `patch2.000.tiger`) phải chép thật, không dùng liên kết cứng.
- **So với app cũ:** chạy code Python trong `tham_khao\DichGame` bằng `cong_cu\python`, với `APPDATA` trỏ vào dữ liệu thử, rồi so kết quả từng byte. GĐ4–5 đã khớp.

**Giao diện**
- Phím giả lập từ shell không tới được bộ bắt phím của app. Dùng tùy chọn khởi động `--dich` / `--f12`, và bấm chuột bằng `SendInput`.

## Dùng độc lập (GĐ8, chưa commit)

Mục tiêu: thao tác hoàn toàn trên app, không cần Claude Code; Claude API chỉ là tùy chọn (mặc định dịch offline).
- **Sửa bản dịch** (`trang_sua_dich.rs`): sửa từng câu, tìm & thay hàng loạt, từ điển `tu_dien.json` (tên riêng + sửa bản dịch) áp cho mọi lần dịch sau (`dich::ap_dung_sua` trong `viec_dich`).
- **Làm tự động** (`tu_dong.rs` + `trang_tu_dong.rs`): một nút dịch → lồng tiếng → cài cho CD / Rise / TR2013 / Shadow / game khác (dò file chữ). Đã chạy thật trên game giả (10 câu dịch offline rồi ghi).
- **Lồng tiếng chung** (`long_tieng_chung.rs` + `trang_long_tieng.rs`): game có file thoại rời wav/ogg/mp3/flac/opus; ghép chữ theo tên file hoặc CSV, giọng nhái gốc hoặc từ kho, mã hóa lại bằng ffmpeg, cài/gỡ có sao lưu. Đã thử thật 3 file ogg, gỡ trả đúng từng byte. Chưa thử trên game thật.
- **Đóng gói**: `scripts\dong_goi.ps1` → `dist\VietHoa_<ver>.zip` (exe + scripts + hướng dẫn, 5,6 MB). Script nhúng trong exe, tự chép ra `scripts\` khi chạy ở máy khác (thư mục không có Cargo.toml). VieNeu fallback sang `cong_cu\python` nếu máy không có app VieNeu. Chưa có tự cập nhật.
## Cập nhật + tiến trình (GĐ9)
- **Cập nhật** (`cap_nhat.rs`): app kiểm tra ngầm bản mới trên GitHub Releases `Zinit-mid/Viet-hoa` (thanh bên báo "Có bản mới"), Cài đặt > CẬP NHẬT tải `VietHoa.exe`, đổi tên exe đang chạy thành `.cu`, mở lại. Phát hành: tăng version trong `Cargo.toml`, commit, `git tag vX.Y.Z`, `git push origin vX.Y.Z` → `.github/workflows/phat_hanh.yml` build + đính kèm `VietHoa.exe` và zip. Đã thử thật v0.1.0 → v0.1.1.
- **Thanh tiến trình toàn cục** (`tien_trinh.rs`): ở đầu cửa sổ mọi trang khi có việc nền (tên, thời gian, %, nút Dừng; chưa biết % thì thanh chạy). Trang mới có việc nền: gọi `tien_trinh::dang_ky(...)` trong `giao_dien::App::moi`.
- Exe người dùng chạy là `D:\Việt hóa\VietHoa.exe` (lối tắt Desktop trỏ vào đây): sau khi build phải chép `target\release\VietHoa.exe` đè lên (hoặc dùng nút cập nhật).

## Thuyết minh phim (GĐ10)
`thuyet_minh.rs` + `scripts\thuyet_minh.py` + thẻ THUYẾT MINH trong trang Phụ đề phim: đọc phụ đề Việt bằng 1 trong 6 giọng VieNeu dựng sẵn (Thiện Minh, Thái Sơn, Đức Trí, Ngọc Linh, Mỹ Duyên, Thục Đoan), có cảm xúc bằng 2 cách kết hợp: (1) nhấn nhá theo câu (đoán cảm xúc từ dấu câu/từ khóa → to nhỏ, nhanh chậm, cao thấp), (2) bộ giọng 3 mẫu bình thường/gấp rút/buồn cho mỗi giọng (`du_lieu\giong_thuyet_minh\<giọng>\`). Đặt đúng thời điểm, ép nhịp, trộn bằng ffmpeg (sidechain hạ tiếng gốc) ra `<phim>_thuyetminh.mp4` + `_giong.m4a`. Đã thử thật 75 giây phim: 7 dòng, đúng thời điểm, hình giữ nguyên. Giới hạn: VieNeu không có nút chỉnh cảm xúc thật nên mức "diễn" vừa phải; chưa nghe đánh giá bằng tai.

## Giọng mẫu (menu chính)
`trang_giong_mau.rs`: thư viện giọng (kho `giong_rieng`): thêm nhiều file / cả thư mục cùng lúc (`clone_giong::nhap_nhieu`), bộ 3 mẫu `Tên__1_binh_thuong / __2_gap_rut / __3_buon` tự gom thành một giọng có cảm xúc (`Tên.wav` + `Tên_gap_rut.wav` + `Tên_buon.wav`; mẫu số `chinh` làm giọng chính, mặc định 2), thêm giọng dựng sẵn VieNeu vào kho, nghe, xóa. Thuyết minh phim chọn được giọng trong kho.

## Giao diện mới (theo GiaoDien_VietHoa_ChoClaudeCode.md, dựng bằng egui)
- `kieu.rs`: nền tối + 3 vệt sáng (tắt được ở Cài đặt > Giao diện), mặt kính, font IBM Plex nhúng trong exe (`assets/fonts/IBMPlex*.ttf`, OFL), cỡ chữ ≥ 14, nút viên thuốc, **nút tiến độ đổ màu theo % thật** (`kieu::nut_tien_do(ui, khoa, nhãn, động từ, màu, bật)` đọc từ `tien_trinh`), công tắc, chip, khối kính (`kieu::khoi`). Màu: Việt hóa = tím, Lồng tiếng = cam, Phụ đề = vàng, Dò file = xanh biển.
- Khung (`giao_dien.rs`): thanh trên (logo + 6 mục lớn + nút Dịch cả màn hình), cột trái "Mục con" (bấm = cuộn tới khối), giữa = trang, cột phải "Việc đang chạy" + "Kho giọng" (ẩn khi cửa sổ < 1280 điểm, thay bằng dải đầu cửa sổ).
- Lưu ý egui: `Color32::from_rgba_unmultiplied` nhân alpha trong không gian tuyến tính nên màu mờ bị sáng; dùng `kieu::rgba(...)`.
- Chưa làm: bảng "Vùng dịch" dạng bảng, 4 thẻ phát sáng ở Dịch màn hình, ô xem trước, blur kính thật (egui không có).

- Icon: `assets/icon.ico` (exe, qua `build.rs` + winresource), `assets/icon_128.rgba` (cửa sổ/taskbar), `assets/icon_64.rgba` (logo thanh trên). Bảng màu thêm tím (vệt sáng, mặt kính, nút, công tắc) cho đỡ nhợt.

## VIỆC TIẾP THEO (làm tuần sau): trình xem video trong app
Mục tiêu: xem phim ngay trong app sau khi đã tạo phụ đề + thuyết minh/lồng tiếng.
- **Cách chọn (đang nghiêng):** nhúng libVLC vào một khung của cửa sổ (HWND con), nạp `cong_cu\vlc\libvlc.dll` bằng `libloading` (~15 hàm FFI, không thêm crate nặng). Thanh điều khiển vẽ bằng egui NGOÀI khung hình (cửa sổ con không bị egui đè lên). Cách 2 (libVLC vẽ từng khung lên texture egui, nút đè lên hình được) nặng hơn; cách 3 (tự giải mã ffmpeg) không đáng.
- **Cần hỏi người dùng:** có cần thanh điều khiển ĐÈ LÊN hình (kiểu YouTube) không? Có → phải dùng cách 2.
- **Việc cần làm trước:** `thuyet_minh.rs` hiện xuất mp4 chỉ 1 đường tiếng đã trộn. Đổi thành mp4 nhiều đường tiếng (gốc / trộn thuyết minh / riêng giọng thuyết minh) + nhúng .srt, để player đổi đường tiếng gọn.
- **Tính năng:** chọn phim đã xử lý từ danh sách; phụ đề Việt bật/tắt + cỡ chữ; đổi tiếng gốc ↔ thuyết minh; tua, tốc độ, toàn màn hình, nhớ vị trí xem dở. Thêm vào mục lớn "Phụ đề phim" (mục con "Xem phim").
- Mở phim hiện dùng VLC ngoài (`phu_de_phim::mo_phim`).

## Trạng thái cuối phiên (24/11/2026)
Giao diện mới (kính tối tím hơi đen, nút neon, thanh trượt, viền đổi màu), trang Giọng mẫu, thuyết minh phim, cập nhật tự động, đã đẩy GitHub (main). Bản phát hành mới nhất là v0.1.1; từ đó đến nay CHƯA gắn tag mới (nút cập nhật chưa thấy giao diện mới) – khi muốn phát hành: tăng version trong Cargo.toml, commit, `git tag vX.Y.Z`, `git push origin vX.Y.Z`.

## v0.1.3 (27/11/2026) – sửa theo báo lỗi
- **Claude bật/tắt:** `claude::co_khoa()` chỉ xét có mã + mã chưa bị từ chối (Việt hóa, phụ đề phim dùng cái này); `claude::co_bat()` thêm công tắc BAT, chỉ dùng cho dịch màn hình trực tiếp. `ly_do_khong_dung()` cho thông báo đúng.
- **Dịch chừa câu:** `ViecDich::chay_claude` chạy 3 vòng (lô 40 → lô 8 có nhắc thẻ → từng câu); `chay_offline` thử lại câu hỏng bằng Google (vẫn giữ thẻ). `viec.loi` = số câu còn lại (không cộng dồn); danh sách ghi ở `cau_chua_dich.txt` cạnh bộ nhớ dịch của game.
- **Phụ đề phim:** nút Dừng giết tiến trình Python ngay (luồng canh riêng); Whisper nạp `local_files_only` trước (không hỏi mạng); tải YouTube thử 3 cách, tự bỏ cách bị kẹt 45 giây.
- **Bản đồ Crimson Desert** (`trang_ban_do.rs`, chuyển từ ban_do_cd.py) + nút **Xuất bảng thú cưỡi** (`crimson_desert::xuat_bang_thu_cuoi`, chỉ đọc game).
- **Chưa làm:** mod "dễ bắt ngựa" (timeScale trong `rodeominigameview` nhóm 0012) chưa có trong app – cần file `ui_rodeominigameview*.uibinary/.html` xuất ra để vá; app cũ cũng chưa có.
- Phát hành: đẩy `main` có version mới trong Cargo.toml là workflow tự build + đăng Release (không cần tag).

## v0.4.0 (28/11/2026) – AI local thông minh hơn
- Dịch / soát bản dịch gửi kèm `huong_dan` của game/phim, tên riêng (BangTen + tu_dien.json) và ví dụ văn phong. Chưa có tên người nói ở đường dịch này.
- Dùng `format` (JSON schema) của Ollama; lỗi HTTP thì gọi lại kiểu thường.
- Trợ lý chat + giải thích lỗi đọc `src/huong_dan_chat.md` (nhúng trong exe). Đổi tên nút thì sửa file này.
- Bật suy luận cho chia người nói và giải thích lỗi, hỏng thì tự gọi lại không suy luận. num_ctx chung 8192.
