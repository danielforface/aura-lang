$ErrorActionPreference = 'Stop'

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..\..') | Select-Object -ExpandProperty Path
$fixtures = Join-Path $repoRoot 'tests\compat'

if (-not (Test-Path $fixtures)) {
  throw "Missing compat fixtures at $fixtures"
}

Write-Host "== Aura compat suite =="

Push-Location $repoRoot
try {
  $files = Get-ChildItem -LiteralPath $fixtures -Filter '*.aura' | Select-Object -ExpandProperty FullName
  if (-not $files -or $files.Count -eq 0) {
    throw "No compat fixtures (*.aura) found in $fixtures"
  }

  foreach ($f in $files) {
    Write-Host "-- build (avm): $f"
    cargo run -q -p aura -- build "$f" --mode avm
  }

  Write-Host "OK"
} finally {
  Pop-Location
}
