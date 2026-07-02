param(
    [string]$BinaryPath = "target/release/uniseq-app.exe",
    [string]$ConfigPath = "src-tauri/tauri.conf.json",
    [string]$OutputDir = "dist"
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$binaryFullPath = Join-Path $repoRoot $BinaryPath
$configFullPath = Join-Path $repoRoot $ConfigPath
$outputFullDir = Join-Path $repoRoot $OutputDir

if (-not (Test-Path -LiteralPath $binaryFullPath)) {
    throw "Binary not found: $binaryFullPath. Build the app first."
}

if (-not (Test-Path -LiteralPath $configFullPath)) {
    throw "Config not found: $configFullPath"
}

$config = Get-Content -LiteralPath $configFullPath -Raw | ConvertFrom-Json
$version = $config.version

if ([string]::IsNullOrWhiteSpace($version)) {
    throw "Could not read version from $configFullPath"
}

New-Item -ItemType Directory -Force -Path $outputFullDir | Out-Null

$tempDir = Join-Path $outputFullDir "portable-temp"
if (Test-Path -LiteralPath $tempDir) {
    Remove-Item -LiteralPath $tempDir -Recurse -Force
}
New-Item -ItemType Directory -Path $tempDir | Out-Null

$portableExePath = Join-Path $tempDir "Uniseq.exe"
Copy-Item -LiteralPath $binaryFullPath -Destination $portableExePath -Force

$zipName = "Uniseq_${version}_windows_x64_portable.zip"
$zipPath = Join-Path $outputFullDir $zipName

if (Test-Path -LiteralPath $zipPath) {
    Remove-Item -LiteralPath $zipPath -Force
}

Compress-Archive -Path $portableExePath -DestinationPath $zipPath -Force
Remove-Item -LiteralPath $tempDir -Recurse -Force

Write-Output $zipPath
