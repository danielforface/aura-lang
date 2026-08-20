cd ..\..\

$Commit = git rev-parse --short HEAD
$Artifact = "aura-site-$Commit.zip"

Remove-Item ".\$Artifact" -Force -ErrorAction SilentlyContinue

tar.exe -a -c `
  -f ".\$Artifact" `
  -C ".\website\out" `
  .

Get-FileHash ".\$Artifact" -Algorithm SHA256
