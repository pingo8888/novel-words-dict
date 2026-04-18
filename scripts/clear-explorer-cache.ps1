$ErrorActionPreference = "SilentlyContinue"

Write-Host "Stopping Explorer..."
Stop-Process -Name explorer -Force
Start-Sleep -Milliseconds 800

$targets = @(
  "$env:LOCALAPPDATA\IconCache.db",
  "$env:LOCALAPPDATA\Microsoft\Windows\Explorer\iconcache*",
  "$env:LOCALAPPDATA\Microsoft\Windows\Explorer\thumbcache*"
)

Write-Host "Removing icon/thumb cache files..."
foreach ($target in $targets) {
  Remove-Item -Path $target -Force -ErrorAction SilentlyContinue
}

Start-Sleep -Milliseconds 500
Write-Host "Restarting Explorer..."
Start-Process explorer.exe

Write-Host "Done: Explorer icon/thumb cache cleared and Explorer restarted."
