# Authenticode signing shared by build_installer.ps1 and hub\package.ps1 (dot-source it, then call Invoke-Sign <file>).
# SHA-256 + RFC 3161 timestamp, so the signature stays valid after the certificate expires; every file is verified.
#
# Default: Azure Artifact Signing, account and profile in signing\azure-metadata.json. The signing client
# (NuGet Microsoft.ArtifactSigning.Client, pinned version and SHA-256) is downloaded once to
# %LOCALAPPDATA%\LoonarSigning. It signs as whoever is signed in to Azure (DefaultAzureCredential): "az login"
# (Azure CLI) on the author's computer, or AZURE_TENANT_ID + AZURE_CLIENT_ID + AZURE_CLIENT_SECRET of an app
# registration in a pipeline. That identity needs the role "Artifact Signing Certificate Profile Signer".
#
# Overrides (environment variables):
#   LOONAR_SIGN_THUMBPRINT  sign with a certificate from the Windows certificate store instead (e.g. Certum)
#   LOONAR_SIGN_DLIB        path to Azure.CodeSigning.Dlib.dll (instead of the downloaded one)
#   LOONAR_SIGN_METADATA    path to another metadata.json
#   LOONAR_SIGN_TIMESTAMP   timestamp server

$script:SignClientVersion = '1.0.146'
$script:SignClientSha256 = '7761e5d8f31e54b764da21d33cd8b0a2940e1b05526c7dfcb5e94b0431426f70'

function Find-SignTool {
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $tool = Get-ChildItem $kits -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\' } | Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $tool) { throw 'signtool.exe not found (Windows SDK: Windows Kits\10\bin\<version>\x64)' }
    $tool.FullName
}

# The Artifact Signing client library for signtool /dlib (downloaded once, checked against the pinned hash).
function Get-SignDlib {
    if ($env:LOONAR_SIGN_DLIB) { return $env:LOONAR_SIGN_DLIB }
    $dir = Join-Path $env:LOCALAPPDATA "LoonarSigning\artifactsigning-$script:SignClientVersion"
    $dll = Join-Path $dir 'bin\x64\Azure.CodeSigning.Dlib.dll'
    if (Test-Path $dll) { return $dll }
    New-Item -ItemType Directory -Force $dir | Out-Null
    $package = Join-Path $dir 'client.zip'
    $url = "https://api.nuget.org/v3-flatcontainer/microsoft.artifactsigning.client/$script:SignClientVersion/microsoft.artifactsigning.client.$script:SignClientVersion.nupkg"
    Invoke-WebRequest $url -OutFile $package -UseBasicParsing
    $hash = (Get-FileHash $package -Algorithm SHA256).Hash.ToLower()
    if ($hash -ne $script:SignClientSha256) {
        Remove-Item $package -Force
        throw "the signing client package has an unexpected SHA-256 ($hash)"
    }
    Expand-Archive $package -DestinationPath $dir -Force
    Remove-Item $package -Force
    if (-not (Test-Path $dll)) { throw "missing $dll in the signing client package" }
    $dll
}

function Invoke-Sign([string]$file) {
    $signtool = Find-SignTool
    if ($env:LOONAR_SIGN_THUMBPRINT) {
        $ts = if ($env:LOONAR_SIGN_TIMESTAMP) { $env:LOONAR_SIGN_TIMESTAMP } else { 'http://time.certum.pl' }
        & $signtool sign /fd SHA256 /tr $ts /td SHA256 /sha1 $env:LOONAR_SIGN_THUMBPRINT $file
    } else {
        $ts = if ($env:LOONAR_SIGN_TIMESTAMP) { $env:LOONAR_SIGN_TIMESTAMP } else { 'http://timestamp.acs.microsoft.com' }
        $metadata = if ($env:LOONAR_SIGN_METADATA) { $env:LOONAR_SIGN_METADATA } else { Join-Path $PSScriptRoot 'azure-metadata.json' }
        & $signtool sign /v /fd SHA256 /tr $ts /td SHA256 /dlib (Get-SignDlib) /dmdf $metadata $file
    }
    if ($LASTEXITCODE -ne 0) { throw "signing $file failed (Azure: is 'az login' done and does the account have the Signer role?)" }
    & $signtool verify /pa $file | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "signature of $file failed verification" }
    "Signed: $file"
}
