# Builds Compass Hub for Windows and packs it: dist\CompassHub-<version>-windows-x64.zip (+ .sha256).
# Also writes the third-party notices for Windows and Linux (the Docker image copies the Linux one).
#   .\hub\package.ps1            build and pack
#   .\hub\package.ps1 -SkipBuild pack the program already built in hub\target\release
#   .\hub\package.ps1 -Sign      public release: signs compass-hub.exe (signing\sign.ps1; "az login" first)
param([switch]$SkipBuild, [switch]$Sign)
$ErrorActionPreference = 'Stop'
$hub = $PSScriptRoot
$root = Split-Path $hub -Parent
$version = (Select-String -Path (Join-Path $hub 'Cargo.toml') -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

Push-Location $hub
try {
    if (-not $SkipBuild) {
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    }
    # Third-party notices of the crates linked into each binary.
    cargo fetch --locked --target x86_64-unknown-linux-gnu | Out-Null
    foreach ($target in 'x86_64-pc-windows-msvc', 'x86_64-unknown-linux-gnu') {
        $meta = Join-Path $env:TEMP "compass-hub-meta-$target.json"
        $json = cargo metadata --format-version 1 --locked --filter-platform $target
        [IO.File]::WriteAllText($meta, ($json -join "`n"), (New-Object Text.UTF8Encoding $false))
        if ($LASTEXITCODE -ne 0) { throw "cargo metadata ($target) failed" }
        $name = if ($target -like '*windows*') { 'windows' } else { 'linux' }
        python -I (Join-Path $root 'tools\gen_notices.py') $meta $meta (Join-Path $hub "THIRD-PARTY-NOTICES-$name.txt")
        if ($LASTEXITCODE -ne 0) { throw 'gen_notices.py failed' }
    }
} finally {
    Pop-Location
}

$stage = Join-Path $env:TEMP "compass-hub-$version"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $hub 'target\release\compass-hub.exe') $stage
if ($Sign) {
    . (Join-Path $root 'signing\sign.ps1')
    Invoke-Sign (Join-Path $stage 'compass-hub.exe')
}
Copy-Item (Join-Path $hub 'README.md'), (Join-Path $hub 'hub.example.json'), (Join-Path $hub 'shop.example.properties') $stage
Copy-Item (Join-Path $root 'LICENSE.en.txt') (Join-Path $stage 'LICENSE.txt')
Copy-Item (Join-Path $hub 'THIRD-PARTY-NOTICES-windows.txt') (Join-Path $stage 'THIRD-PARTY-NOTICES.txt')
$dist = Join-Path $root 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$zip = Join-Path $dist "CompassHub-$version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
"$hash  $(Split-Path $zip -Leaf)" | Out-File -Encoding ascii "$zip.sha256"
Remove-Item -Recurse -Force $stage
Write-Host "Done: $zip"
Write-Host "SHA-256: $hash"
