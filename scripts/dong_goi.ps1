# Dong goi app Viet Hoa thanh 1 file zip de chep sang may khac (khong can Rust / Python tren may do).
# Chay: powershell -ExecutionPolicy Bypass -File scripts\dong_goi.ps1
# Ket qua: dist\VietHoa_<phien ban>.zip = VietHoa.exe + scripts\ + HUONG_DAN.txt
$ErrorActionPreference = 'Stop'
$Goc = Split-Path -Parent $PSScriptRoot
Set-Location $Goc

$env:CARGO_HOME  = Join-Path $Goc 'cong_cu\cargo'
$env:RUSTUP_HOME = Join-Path $Goc 'cong_cu\rustup'
$env:PATH = (Join-Path $Goc 'cong_cu\cargo\bin') + ';' + $env:PATH

Write-Host '1. Build ban release'
cargo build --release
if ($LASTEXITCODE -ne 0) { throw 'build loi' }

$ver = ((Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value)
$dist = Join-Path $Goc 'dist'
$thu = Join-Path $dist "VietHoa_$ver"
Remove-Item -Recurse -Force $thu -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $thu, "$thu\scripts" | Out-Null

Write-Host '2. Chep file'
Copy-Item 'target\release\VietHoa.exe' $thu
Copy-Item 'scripts\*' "$thu\scripts"
Remove-Item "$thu\scripts\dong_goi.ps1" -ErrorAction SilentlyContinue
@"
VIET HOA $ver - cong cu dich + long tieng game / phim

CAI DAT LAN DAU (may moi):
 1. Giai nen thu muc nay vao noi khong can quyen admin (vd D:\VietHoa).
 2. Bam dup VietHoa.exe. Vao Cai dat > "Tai cong cu con thieu" (hoac chay
    scripts\tai_cong_cu.ps1): tu tai ffmpeg, vgmstream, VLC, Python rieng, mo hinh dich offline,
    Whisper, VieNeu. Can mang, ~10 GB neu co card NVIDIA (bo qua GPU: them -BoQuaGpu).
 3. Crimson Desert can them Wwise (Audiokinetic Launcher) va DMM de nap mod - khong tu tai duoc.

DU LIEU: moi thu nam trong thu muc nay (du_lieu\, cong_cu\), khong ghi vao %APPDATA% hay o C.
CHUYEN SANG MAY KHAC: chep ca thu muc (cong_cu\ va du_lieu\ nam trong do) la chay duoc.
"@ | Set-Content -Encoding UTF8 "$thu\HUONG_DAN.txt"

Write-Host '3. Nen zip'
$zip = Join-Path $dist "VietHoa_$ver.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path "$thu\*" -DestinationPath $zip
$mb = [math]::Round((Get-Item $zip).Length / 1MB, 1)
Write-Host "XONG: $zip ($mb MB)"
