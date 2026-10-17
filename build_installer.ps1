# Builds the application release and the Loonar Compass installer.
#   .\build_installer.ps1           release build: application + installer (installs per user by default, no admin needed)
#   .\build_installer.ps1 -SkipApp  installer only, with the application already in target\release
#   .\build_installer.ps1 -Sign     public release: signs the application and the installer (no signing configuration = error)
# Output: dist\LoonarCompass-Setup-<version>.exe and its .sha256 file. The version comes from Cargo.toml
# (shown as major.minor, plus .patch when it is not 0 - the same rule the installer uses).
#
# Digital signature (Authenticode) - configured through environment variables:
#   certificate in the Windows certificate store (e.g. Certum SimplySign):
#     LOONAR_SIGN_THUMBPRINT  SHA-1 thumbprint of the code-signing certificate
#     LOONAR_SIGN_TIMESTAMP   timestamp server (default http://time.certum.pl)
#   Azure Artifact Signing:
#     LOONAR_SIGN_DLIB        path to Azure.CodeSigning.Dlib.dll
#     LOONAR_SIGN_METADATA    path to metadata.json (Endpoint, CodeSigningAccountName, CertificateProfileName)
#     LOONAR_SIGN_TIMESTAMP   default http://timestamp.acs.microsoft.com
param([switch]$SkipApp, [switch]$Sign)
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

# [package] version from a Cargo.toml file.
function Get-CargoVersion([string]$toml) {
    $inPackage = $false
    foreach ($line in [IO.File]::ReadAllLines($toml)) {
        if ($line -match '^\s*\[(.+)\]\s*$') { $inPackage = ($Matches[1].Trim() -eq 'package'); continue }
        if ($inPackage -and $line -match '^\s*version\s*=\s*"([^"]+)"') { return $Matches[1] }
    }
    throw "no [package] version in $toml"
}

$fullVersion = Get-CargoVersion (Join-Path $root 'Cargo.toml')
$setupVersion = Get-CargoVersion (Join-Path $root 'installer\Cargo.toml')
if ($setupVersion -ne $fullVersion) {
    throw "version mismatch: Cargo.toml has $fullVersion, installer\Cargo.toml has $setupVersion (set both to the same version)"
}
$version = $fullVersion
if ($fullVersion -match '^(\d+)\.(\d+)\.(\d+)(.*)$' -and $Matches[3] -eq '0') { $version = "$($Matches[1]).$($Matches[2])$($Matches[4])" }

function Find-SignTool {
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $tool = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $tool) { throw 'signtool.exe not found (Windows SDK: Windows Kits\10\bin\<version>\x64)' }
    $tool.FullName
}

# Signs a file (SHA-256 + RFC 3161 timestamp, so the signature stays valid after the certificate expires).
function Invoke-Sign([string]$file) {
    if (-not $Sign) { return }
    $signtool = Find-SignTool
    if ($env:LOONAR_SIGN_DLIB) {
        $ts = if ($env:LOONAR_SIGN_TIMESTAMP) { $env:LOONAR_SIGN_TIMESTAMP } else { 'http://timestamp.acs.microsoft.com' }
        & $signtool sign /v /fd SHA256 /tr $ts /td SHA256 /dlib $env:LOONAR_SIGN_DLIB /dmdf $env:LOONAR_SIGN_METADATA $file
    } elseif ($env:LOONAR_SIGN_THUMBPRINT) {
        $ts = if ($env:LOONAR_SIGN_TIMESTAMP) { $env:LOONAR_SIGN_TIMESTAMP } else { 'http://time.certum.pl' }
        & $signtool sign /fd SHA256 /tr $ts /td SHA256 /sha1 $env:LOONAR_SIGN_THUMBPRINT $file
    } else {
        throw 'signing: set LOONAR_SIGN_THUMBPRINT or LOONAR_SIGN_DLIB + LOONAR_SIGN_METADATA'
    }
    if ($LASTEXITCODE -ne 0) { throw "signing $file failed" }
    & $signtool verify /pa $file | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "signature of $file failed verification" }
    "Signed: $file"
}

function Invoke-Cargo([string]$dir, [string[]]$cargoArgs) {
    Push-Location $dir
    try {
        & cmd /c "cargo $($cargoArgs -join ' ') 2>&1" | Select-String '^error|^warning: unused|Finished' -Context 0, 10 | ForEach-Object { "$_" }
        if ($LASTEXITCODE -ne 0) { throw "cargo $($cargoArgs -join ' ') failed" }
    } finally { Pop-Location }
}

# cargo metadata of the package in $dir, written byte for byte (UTF-8) to $out (cmd redirection: PowerShell 5.1
# would re-encode the output as UTF-16).
function Save-CargoMetadata([string]$dir, [string]$out) {
    Push-Location $dir
    try {
        & cmd /c "cargo metadata --format-version 1 --offline --locked --filter-platform x86_64-pc-windows-msvc > `"$out`" 2> `"$out.log`""
        if ($LASTEXITCODE -ne 0) { throw "cargo metadata in $dir failed: $([IO.File]::ReadAllText("$out.log"))" }
    } finally {
        Remove-Item "$out.log" -ErrorAction SilentlyContinue
        Pop-Location
    }
}

# Fails when a built program contains paths of the build computer (user name, project folder, profile folder),
# in ASCII/UTF-8 or UTF-16LE (at both byte alignments).
function Assert-NoBuildPaths([string]$file) {
    $bytes = [IO.File]::ReadAllBytes($file)
    $texts = @(
        [Text.Encoding]::GetEncoding(28591).GetString($bytes),
        [Text.Encoding]::Unicode.GetString($bytes),
        [Text.Encoding]::Unicode.GetString($bytes, 1, $bytes.Length - 1)
    )
    foreach ($needle in @($env:USERNAME, $root, 'Users\')) {
        foreach ($text in $texts) {
            if ($text.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                throw "$file contains the build path fragment '$needle' (check --remap-path-prefix and the PDB path)"
            }
        }
    }
    "No build paths in $file"
}

function Find-Python {
    foreach ($candidate in @(@('py', '-3'), @('python'))) {
        if (-not (Get-Command $candidate[0] -CommandType Application -ErrorAction SilentlyContinue)) { continue }
        # The Microsoft Store placeholder "python.exe" only prints a hint to stderr: such a candidate is skipped.
        $ErrorActionPreference = 'Continue'
        try {
            & cmd /c "$($candidate -join ' ') -c `"import sys; sys.exit(0 if sys.version_info[0] == 3 else 1)`" >nul 2>nul"
            if ($LASTEXITCODE -eq 0) { return , $candidate }
        } catch {
        } finally {
            $ErrorActionPreference = 'Stop'
        }
    }
    throw 'Python 3 not found (needed for tools\gen_notices.py)'
}

# Build flags: RUSTFLAGS replaces the flags from .cargo\config.toml, so the static C runtime is repeated here.
# The path prefixes keep the build computer's folders out of the programs (panic messages, file!()).
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$flags = @('-C', 'target-feature=+crt-static', "--remap-path-prefix=$cargoHome=C:\cargo", "--remap-path-prefix=$root=C:\build")
$savedRustFlags = $env:RUSTFLAGS
$savedEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
if (($cargoHome + $root).Contains(' ')) {
    # A path with a space would split RUSTFLAGS; the encoded variable keeps each flag whole.
    Remove-Item Env:\RUSTFLAGS -ErrorAction SilentlyContinue
    $env:CARGO_ENCODED_RUSTFLAGS = $flags -join [char]0x1f
} else {
    Remove-Item Env:\CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue
    $env:RUSTFLAGS = "-C target-feature=+crt-static --remap-path-prefix=$cargoHome=C:\cargo --remap-path-prefix=$root=C:\build"
}

try {
    # Third-party notices: the application's About view embeds them, so they exist before the application is built.
    'Generating THIRD-PARTY-NOTICES.txt...'
    $metaApp = Join-Path $env:TEMP 'loonar-compass-meta-app.json'
    $metaInst = Join-Path $env:TEMP 'loonar-compass-meta-inst.json'
    $notices = Join-Path $root 'THIRD-PARTY-NOTICES.txt'
    Save-CargoMetadata $root $metaApp
    Save-CargoMetadata (Join-Path $root 'installer') $metaInst
    $python = Find-Python
    Push-Location $root
    try {
        & $python[0] @($python | Select-Object -Skip 1) -I (Join-Path $root 'tools\gen_notices.py') $metaApp $metaInst $notices
        if ($LASTEXITCODE -ne 0) { throw 'tools\gen_notices.py failed' }
    } finally {
        Pop-Location
        Remove-Item $metaApp, $metaInst -ErrorAction SilentlyContinue
    }

    if (-not $SkipApp) {
        'Building the application (release)...'
        Invoke-Cargo $root @('build', '--release')
    }
    $app = Join-Path $root 'target\release\loonar-compass.exe'
    if (-not (Test-Path $app)) { throw "missing $app" }
    Assert-NoBuildPaths $app

    # Installer contents (the names installer\build.rs checks): a fresh folder, so no stale file is embedded.
    $payload = Join-Path $root 'installer\payload'
    if (Test-Path $payload) { Remove-Item (Join-Path $payload '*') -Recurse -Force }
    New-Item -ItemType Directory -Force $payload | Out-Null
    Copy-Item $app (Join-Path $payload 'app.exe') -Force
    # The application is embedded in the installer, so it must be signed before the installer is built.
    Invoke-Sign (Join-Path $payload 'app.exe')
    Copy-Item $notices (Join-Path $payload 'THIRD-PARTY-NOTICES.txt') -Force
    # License and changelog in 4 languages (pl = the base files without a language suffix); the user guide is on the website.
    foreach ($lang in 'pl', 'en', 'de', 'fr') {
        $suffix = if ($lang -eq 'pl') { '' } else { ".$lang" }
        $sources = @{ "LICENSE.$lang.txt" = "LICENSE$suffix.txt"; "CHANGELOG.$lang.md" = "CHANGELOG$suffix.md" }
        foreach ($target in $sources.Keys) {
            $source = Join-Path $root $sources[$target]
            if (-not (Test-Path $source)) { throw "missing $source" }
            Copy-Item $source (Join-Path $payload $target) -Force
        }
    }

    'Building the installer...'
    Invoke-Cargo (Join-Path $root 'installer') @('build', '--release')

    $dist = Join-Path $root 'dist'
    New-Item -ItemType Directory -Force $dist | Out-Null
    $name = "LoonarCompass-Setup-$version.exe"
    $out = Join-Path $dist $name
    Copy-Item (Join-Path $root 'installer\target\release\loonar-compass-setup.exe') $out -Force
    try {
        Assert-NoBuildPaths $out
    } catch {
        Remove-Item $out -Force -ErrorAction SilentlyContinue
        throw
    }
    # The uninstaller is a byte-identical copy of the installer, so it carries the same signature.
    Invoke-Sign $out
    $item = Get-Item $out
    "Done: $($item.FullName) ($([math]::Round($item.Length / 1MB, 1)) MB)"
    $hash = (Get-FileHash $out -Algorithm SHA256).Hash.ToLower()
    # Checksum to publish next to the file (sha256sum format).
    [IO.File]::WriteAllText("$out.sha256", "$hash *$name`n")
    "SHA-256: $hash"
} finally {
    if ($null -ne $savedRustFlags) { $env:RUSTFLAGS = $savedRustFlags } else { Remove-Item Env:\RUSTFLAGS -ErrorAction SilentlyContinue }
    if ($null -ne $savedEncodedFlags) { $env:CARGO_ENCODED_RUSTFLAGS = $savedEncodedFlags } else { Remove-Item Env:\CARGO_ENCODED_RUSTFLAGS -ErrorAction SilentlyContinue }
}
