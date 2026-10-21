# Registers the EA add-in for the current user only (no administrator rights): COM classes in HKCU\Software\Classes
# (64- and 32-bit views) and the add-in entry EA reads at start. -Unregister removes everything again.
param([string]$Dll = (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'LoonarCompass.EAAddin.dll'), [switch]$Unregister)
$ErrorActionPreference = 'Stop'
$classes = @(
    @{ ProgId = 'LoonarCompass.Addin'; Class = 'LoonarCompass.Addin'; Clsid = '{6C1E5B7A-3D2F-4B8E-9A41-2F7C0D5E8B13}' },
    @{ ProgId = 'LoonarCompass.Tab'; Class = 'LoonarCompass.CompassTab'; Clsid = '{9B4D2E61-7A3C-4F15-8E2B-5D0C6A1F9E47}' }
)
$root = 'HKCU:\Software\Classes'
$addins = @('HKCU:\Software\Sparx Systems\EAAddins64\LoonarCompass', 'HKCU:\Software\Sparx Systems\EAAddins\LoonarCompass')
function Set-Default($path, $value) { New-Item -Path $path -Force | Out-Null; Set-ItemProperty -Path $path -Name '(default)' -Value $value }
if ($Unregister) {
    foreach ($c in $classes) {
        foreach ($p in @("$root\$($c.ProgId)", "$root\CLSID\$($c.Clsid)", "$root\Wow6432Node\CLSID\$($c.Clsid)")) { if (Test-Path $p) { Remove-Item $p -Recurse -Force } }
    }
    foreach ($a in $addins) { if (Test-Path $a) { Remove-Item $a -Recurse -Force } }
    Write-Host 'Unregistered.'
    return
}
$Dll = (Resolve-Path $Dll).Path
# Not URL-escaped: with %20 the .NET loader does not find a folder with spaces.
$codeBase = 'file:///' + $Dll.Replace('\', '/')
$assembly = [Reflection.AssemblyName]::GetAssemblyName($Dll).FullName
foreach ($c in $classes) {
    Set-Default "$root\$($c.ProgId)" $c.Class
    Set-Default "$root\$($c.ProgId)\CLSID" $c.Clsid
    foreach ($clsidRoot in @("$root\CLSID", "$root\Wow6432Node\CLSID")) {
        $k = "$clsidRoot\$($c.Clsid)"
        Set-Default $k $c.Class
        Set-Default "$k\ProgId" $c.ProgId
        Set-Default "$k\InprocServer32" 'mscoree.dll'
        Set-ItemProperty "$k\InprocServer32" -Name ThreadingModel -Value 'Both'
        Set-ItemProperty "$k\InprocServer32" -Name Class -Value $c.Class
        Set-ItemProperty "$k\InprocServer32" -Name Assembly -Value $assembly
        Set-ItemProperty "$k\InprocServer32" -Name RuntimeVersion -Value 'v4.0.30319'
        Set-ItemProperty "$k\InprocServer32" -Name CodeBase -Value $codeBase
        New-Item -Path "$k\Implemented Categories\{62C8FE65-4EBB-45e7-B440-6E39B2CDBF29}" -Force | Out-Null
    }
}
foreach ($a in $addins) { Set-Default $a 'LoonarCompass.Addin' }
Write-Host "Registered: $Dll"
