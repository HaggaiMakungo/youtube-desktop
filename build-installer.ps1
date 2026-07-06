#!/usr/bin/env pwsh
<#
  Build, sign, and package YouTube Desktop installer
  This is the nuclear option that makes everything work
  Usage: .\build-installer.ps1
  
  What this actually does:
  1. Signs youtube-desktop.exe so Windows doesn't cry about it
  2. Tells Inno Setup to bundle everything into an installer
  3. Signs the installer so people don't get scared warnings
#>

# if ANYTHING fails, bail immediately — we don't do partial installs around here
$ErrorActionPreference = "Stop"

# Paths
$ProjectRoot = Split-Path $MyInvocation.MyCommand.Path -Parent
$SignTool = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe"
$ISCC = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
$PfxFile = "$ProjectRoot\signing\youtube-desktop.pfx"
$AppExe = "$ProjectRoot\src-tauri\target\release\youtube-desktop.exe"
$SetupScript = "$ProjectRoot\setup.iss"
$SetupExe = "$ProjectRoot\installer\YouTube-Desktop-Setup.exe"

# grab the password from the env so we don't have to type it every goddamn time
# if it's not there, fall back to a prompt — inconvenient but not the end of the world
$PfxPassword = $env:YT_DESKTOP_PFX_PASSWORD
if (-not $PfxPassword) {
  Write-Host "PFX password not found in YT_DESKTOP_PFX_PASSWORD environment variable."
  $PfxPassword = Read-Host -AsSecureString "Enter PFX password"
  # convert SecureString to plain text so signtool can actually use it
  # yes this is a bit cursed but it never leaves memory so it's fine
  $PfxPassword = [Runtime.InteropServices.Marshal]::PtrToStringAuto([Runtime.InteropServices.Marshal]::SecureStringToCoTaskMemUnicode($PfxPassword))
}

Write-Host ""
Write-Host "=====================================================" -ForegroundColor Cyan
Write-Host "  Building & Signing YouTube Desktop Installer" -ForegroundColor Cyan
Write-Host "=====================================================" -ForegroundColor Cyan
Write-Host ""

# Step 1: Sign app executable
# unsigned exe = Windows Defender losing its mind and scaring users away. sign the damn thing.
Write-Host "[1/3] Signing youtube-desktop.exe..." -ForegroundColor Yellow
if (-not (Test-Path $AppExe)) {
  # you forgot to build first, didn't you
  Write-Host "ERROR: $AppExe not found. Run 'npm run tauri:build' first." -ForegroundColor Red
  exit 1
}
& $SignTool sign /fd SHA256 /p $PfxPassword /f $PfxFile $AppExe
if ($LASTEXITCODE -ne 0) {
  Write-Host "ERROR: Failed to sign app executable." -ForegroundColor Red
  exit 1
}
Write-Host "SUCCESS: Signed successfully" -ForegroundColor Green
Write-Host ""

# Step 2: Compile installer
# Inno Setup takes our setup.iss and turns it into a proper Windows installer like a civilized tool
Write-Host "[2/3] Compiling Inno Setup installer..." -ForegroundColor Yellow
if (-not (Test-Path $SetupScript)) {
  Write-Host "ERROR: $SetupScript not found." -ForegroundColor Red
  exit 1
}
& $ISCC $SetupScript
if ($LASTEXITCODE -ne 0) {
  Write-Host "ERROR: Inno Setup compilation failed." -ForegroundColor Red
  exit 1
}
Write-Host "SUCCESS: Compiled successfully" -ForegroundColor Green
Write-Host ""

# Step 3: Sign installer
# sign the setup exe too — otherwise SmartScreen calls it a virus and your users never install it
Write-Host "[3/3] Signing YouTube-Desktop-Setup.exe..." -ForegroundColor Yellow
if (-not (Test-Path $SetupExe)) {
  Write-Host "ERROR: $SetupExe not found. Inno Setup build may have failed." -ForegroundColor Red
  exit 1
}
& $SignTool sign /fd SHA256 /p $PfxPassword /f $PfxFile $SetupExe
if ($LASTEXITCODE -ne 0) {
  Write-Host "ERROR: Failed to sign installer." -ForegroundColor Red
  exit 1
}
Write-Host "SUCCESS: Signed successfully" -ForegroundColor Green
Write-Host ""

# Summary
Write-Host "=====================================================" -ForegroundColor Cyan
Write-Host "  Build Complete" -ForegroundColor Cyan
Write-Host "=====================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Output: $SetupExe" -ForegroundColor Green
Write-Host ""
Write-Host "Ready to distribute! Setup.exe is signed and ready." -ForegroundColor Cyan
Write-Host ""
