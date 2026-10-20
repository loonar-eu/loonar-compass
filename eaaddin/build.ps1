# Builds the EA add-in (LoonarCompass.EAAddin.dll) with the .NET Framework 4 compiler that comes with Windows.
$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
$out = Join-Path $here 'LoonarCompass.EAAddin.dll'
& $csc /nologo /target:library /platform:anycpu /optimize+ /out:$out `
    /r:System.Windows.Forms.dll /r:System.Drawing.dll /r:Microsoft.CSharp.dll /r:System.Core.dll `
    (Join-Path $here 'CompassAddin.cs')
if ($LASTEXITCODE -ne 0) { throw "csc failed" }
Write-Host "Built: $out"
