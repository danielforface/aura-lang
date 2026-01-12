param(
  [string]$InstallDir = "",
  [switch]$Force
)

$ErrorActionPreference = 'Stop'

function Write-Info([string]$m) { Write-Host "[AuraSDK] $m" }

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
  $InstallDir = $scriptDir
}

$InstallDir = (Resolve-Path $InstallDir).Path
$BinDir = Join-Path $InstallDir "bin"

if (!(Test-Path $BinDir)) {
  throw "bin/ folder not found at: $BinDir"
}

# Set AURA_HOME (current user)
[Environment]::SetEnvironmentVariable("AURA_HOME", $InstallDir, "User")
Write-Info "Set AURA_HOME=$InstallDir"

# Add bin/ to PATH (current user)
$oldPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($null -eq $oldPath) { $oldPath = "" }
$parts = $oldPath.Split(';') | Where-Object { $_ -and $_.Trim() -ne "" }

if ($parts -notcontains $BinDir) {
  $newPath = ($parts + $BinDir) -join ';'
  [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
  Write-Info "Added to user PATH: $BinDir"
} else {
  Write-Info "bin/ already on user PATH"
}

# Smoke test (non-fatal)
try {
  $auraExe = Join-Path $BinDir "aura.exe"
  if (Test-Path $auraExe) {
    & $auraExe --version | Out-Null
    & $auraExe --help | Out-Null
    Write-Info "Smoke test OK: aura --version / --help"
  } else {
    Write-Info "Smoke test skipped: aura.exe not found (check that bin/ contains the SDK executables)"
  }
} catch {
  Write-Info "Smoke test failed (you may need a new terminal): $($_.Exception.Message)"
}

Write-Info "Done. Open a NEW terminal and run: aura init"
