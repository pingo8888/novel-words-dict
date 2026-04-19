param(
  [string]$Tag
)

$ErrorActionPreference = "Stop"

if (-not $Tag) {
  $pkg = Get-Content -Raw package.json | ConvertFrom-Json
  $Tag = "$($pkg.version)"
}

if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
  $keyPath = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
  if (-not $keyPath) {
    $defaultKeyPath = Join-Path $env:USERPROFILE ".tauri\novel-words-dict.key"
    if (Test-Path -LiteralPath $defaultKeyPath) {
      $keyPath = $defaultKeyPath
    }
  }

  if ($keyPath) {
    if (-not (Test-Path -LiteralPath $keyPath)) {
      Write-Error "私钥文件不存在：$keyPath"
    }
    $env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content -LiteralPath $keyPath -Raw).Trim()
    if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
      Write-Error "私钥文件为空：$keyPath"
    }
    Write-Host "已从私钥文件加载 TAURI_SIGNING_PRIVATE_KEY：$keyPath"
  } else {
    Write-Error "请先设置 TAURI_SIGNING_PRIVATE_KEY，或设置 TAURI_SIGNING_PRIVATE_KEY_PATH（或使用默认路径 $env:USERPROFILE\\.tauri\\novel-words-dict.key）。"
  }
}

Write-Host "==> 1/3 前端构建"
npm run build

Write-Host "==> 2/3 Tauri 打包（跳过重复 beforeBuildCommand）"
npm run tauri build -- --config '{"build":{"beforeBuildCommand":""}}'

$bundleDir = Join-Path "src-tauri\target\release\bundle" "nsis"
$installer = Get-ChildItem -Path $bundleDir -Filter "*-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1

if (-not $installer) {
  Write-Error "未找到安装包：$bundleDir\\*-setup.exe"
}

$sigPath = "$($installer.FullName).sig"
$latestJson = Join-Path $bundleDir "latest.json"

if (-not (Test-Path $sigPath)) {
  Write-Error "未找到签名文件：$sigPath"
}

Write-Host "正在按当前安装包与签名重写 latest.json ..."

$pkg = Get-Content -Raw package.json | ConvertFrom-Json
$version = "$($pkg.version)"
if (-not $version) {
  Write-Error "无法从 package.json 读取版本号。"
}

$origin = (git remote get-url origin).Trim()
$m = [regex]::Match($origin, "github\.com[:/](?<owner>[^/]+)/(?<repo>[^/.]+)(?:\.git)?$")
if (-not $m.Success) {
  Write-Error "无法从 origin 推断 GitHub 仓库，请手动创建 latest.json。origin=$origin"
}

$owner = $m.Groups["owner"].Value
$repo = $m.Groups["repo"].Value
$installerName = $installer.Name
$downloadUrl = "https://github.com/$owner/$repo/releases/download/$Tag/$installerName"
$signature = (Get-Content -LiteralPath $sigPath -Raw).Trim()

if (-not $signature) {
  Write-Error "签名文件为空：$sigPath"
}

$platformInfo = [ordered]@{
  url = $downloadUrl
  signature = $signature
}

$manifest = [ordered]@{
  version = $version
  notes = ""
  pub_date = (Get-Date).ToUniversalTime().ToString("o")
  platforms = [ordered]@{
    "windows-x86_64-nsis" = $platformInfo
    "windows-x86_64" = $platformInfo
  }
}

$manifest | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $latestJson -Encoding utf8NoBOM
Write-Host "已生成：$latestJson"

if (-not (Test-Path $latestJson)) {
  Write-Error "未找到 updater 元数据：$latestJson"
}

Write-Host "==> 3/3 产物检查通过"
Write-Host "Installer : $($installer.FullName)"
Write-Host "Signature : $sigPath"
Write-Host "latest.json: $latestJson"

Write-Host ""
Write-Host "下一步上传到 GitHub Release（tag: $Tag）："
Write-Host "gh release upload $Tag `"$($installer.FullName)`" `"$sigPath`" `"$latestJson`" --clobber"
