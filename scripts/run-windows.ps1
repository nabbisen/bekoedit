# scripts/run-windows.ps1
#
# Unblocks the bekoedit binary from Windows SmartScreen so it can run
# without a paid Authenticode certificate.
#
# Usage (run once in PowerShell after downloading the release archive):
#   Set-ExecutionPolicy -Scope Process -ExecutionPolicy Bypass
#   .\scripts\run-windows.ps1
#
# Or to unblock a specific path:
#   .\scripts\run-windows.ps1 -BinPath "C:\path\to\bekoedit.exe"
#
# What this does:
#   Unblock-File removes the Zone.Identifier alternate data stream that
#   Windows sets on files downloaded from the internet, which triggers
#   SmartScreen. This only affects this file, not Windows security settings.

param(
    [string]$BinPath = ".\bekoedit.exe"
)

if (-not (Test-Path $BinPath)) {
    Write-Error "Binary not found at: $BinPath"
    Write-Host "Usage: .\run-windows.ps1 [-BinPath path\to\bekoedit.exe]"
    exit 1
}

Write-Host "Unblocking $BinPath ..."
Unblock-File -Path $BinPath
Write-Host "Done. You can now launch bekoedit:"
Write-Host "  & '$BinPath'"
