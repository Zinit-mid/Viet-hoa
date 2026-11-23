# Tai cac cong cu phu cho app Viet Hoa vao D:\Viet hoa\cong_cu
# Chay lai nhieu lan duoc: muc nao da co thi bo qua.
param([switch]$BoQuaGpu)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Goc = Split-Path -Parent $PSScriptRoot
$CC  = Join-Path $Goc 'cong_cu'
$Tai = Join-Path $CC 'tai_ve'
New-Item -ItemType Directory -Force $CC, $Tai | Out-Null

function Tai($url, $dich) {
    if (Test-Path $dich) { Write-Host "  da co: $dich"; return }
    Write-Host "  tai: $url"
    Invoke-WebRequest $url -OutFile "$dich.tam" -UseBasicParsing
    Move-Item -Force "$dich.tam" $dich
}
function GiaiNen($zip, $dich) {
    if (Test-Path $dich) { return }
    $tam = "$dich.tam"; Remove-Item -Recurse -Force $tam -ErrorAction SilentlyContinue
    Expand-Archive -Force $zip $tam
    $con = @(Get-ChildItem $tam)
    if ($con.Count -eq 1 -and $con[0].PSIsContainer) { Move-Item $con[0].FullName $dich; Remove-Item -Recurse $tam }
    else { Move-Item $tam $dich }
}

Write-Host '1. Font Be Vietnam Pro'
$Font = Join-Path $Goc 'assets\fonts'; New-Item -ItemType Directory -Force $Font | Out-Null
foreach ($k in 'Regular', 'Bold') {
    Tai "https://github.com/google/fonts/raw/main/ofl/bevietnampro/BeVietnamPro-$k.ttf" (Join-Path $Font "BeVietnamPro-$k.ttf")
}

Write-Host '2. ffmpeg'
Tai 'https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip' "$Tai\ffmpeg.zip"
GiaiNen "$Tai\ffmpeg.zip" "$CC\ffmpeg"

Write-Host '3. vgmstream-cli'
Tai 'https://github.com/vgmstream/vgmstream/releases/latest/download/vgmstream-win64.zip' "$Tai\vgmstream.zip"
GiaiNen "$Tai\vgmstream.zip" "$CC\vgmstream"

Write-Host '4. VLC (ban zip)'
$trang = (Invoke-WebRequest 'https://download.videolan.org/pub/videolan/vlc/last/win64/' -UseBasicParsing).Content
$ten = [regex]::Match($trang, 'vlc-[\d.]+-win64\.zip').Value
if ($ten) { Tai "https://download.videolan.org/pub/videolan/vlc/last/win64/$ten" "$Tai\vlc.zip"; GiaiNen "$Tai\vlc.zip" "$CC\vlc" }

Write-Host '5. Mo hinh dich offline quickmt-en-vi'
$MoHinh = Join-Path $Goc 'du_lieu\mo_hinh\quickmt-en-vi'
New-Item -ItemType Directory -Force $MoHinh | Out-Null
$ds = (Invoke-RestMethod 'https://huggingface.co/api/models/quickmt/quickmt-en-vi').siblings.rfilename
foreach ($f in $ds) {
    if ($f -match '^\.' -or $f -match '/') { continue }
    Tai "https://huggingface.co/quickmt/quickmt-en-vi/resolve/main/$f" (Join-Path $MoHinh $f)
}

Write-Host '6. Python 3.12.10 rieng (cho AI: dich offline, Whisper, VieNeu)'
$Py = Join-Path $CC 'python'
Tai 'https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip' "$Tai\python.zip"
if (-not (Test-Path "$Py\python.exe")) {
    Expand-Archive -Force "$Tai\python.zip" $Py
    Set-Content -Encoding ascii "$Py\python312._pth" "python312.zip`r`n.`r`nLib\site-packages`r`nimport site`r`n"
}
Tai 'https://bootstrap.pypa.io/get-pip.py' "$Tai\get-pip.py"
if (-not (Test-Path "$Py\Scripts\pip.exe")) { & "$Py\python.exe" "$Tai\get-pip.py" --no-warn-script-location }
$pip = @("$Py\python.exe", '-m', 'pip', 'install', '--no-warn-script-location', '--disable-pip-version-check')

Write-Host '7. Thu vien Python: dich offline, phu de phim, giong doc'
& $pip[0] $pip[1..5] ctranslate2 sentencepiece 'faster-whisper>=1.1.0' 'yt-dlp[default]' deno edge-tts

if (-not $BoQuaGpu) {
    Write-Host '8. VieNeu ban GPU (torch 2.8.0 cu128, ~3 GB)'
    & $pip[0] $pip[1..5] torch==2.8.0 torchaudio==2.8.0 --index-url https://download.pytorch.org/whl/cu128
    & $pip[0] $pip[1..5] 'transformers==4.57.6' vieneu
} else {
    Write-Host '8. VieNeu ban CPU'
    & $pip[0] $pip[1..5] vieneu
}
Write-Host 'XONG.'
