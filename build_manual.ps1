# Generates the PDF user guides in 4 languages and the hub installation guide for IT (Microsoft Edge in headless mode):
#   docs\Instrukcja_uzytkownika.html -> .pdf (pl), docs\Manual.<en|de|fr>.html -> .pdf,
#   docs\Hub_Installation_Guide.en/.de/.fr.html and docs\Instalacja_huba.html -> .pdf
$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
$edge = @('C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe', 'C:\Program Files\Microsoft\Edge\Application\msedge.exe') | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Microsoft Edge not found' }
$profile = Join-Path $env:TEMP 'loonar-manual-edge'
foreach ($name in 'Instrukcja_uzytkownika', 'Manual.en', 'Manual.de', 'Manual.fr', 'Hub_Installation_Guide.en', 'Hub_Installation_Guide.de', 'Hub_Installation_Guide.fr', 'Instalacja_huba') {
    $html = Join-Path $root "docs\$name.html"
    $pdf = Join-Path $root "docs\$name.pdf"
    if (-not (Test-Path $html)) { throw "missing $html" }
    $before = if (Test-Path $pdf) { (Get-Item $pdf).LastWriteTime } else { [datetime]::MinValue }
    $url = 'file:///' + $html.Replace('\', '/')
    $p = Start-Process $edge -ArgumentList '--headless', '--disable-gpu', '--no-pdf-header-footer', "--user-data-dir=`"$profile`"", "--print-to-pdf=`"$pdf`"", "`"$url`"" -PassThru
    # Edge may not exit after printing, so we wait for the file, not for the process.
    $deadline = (Get-Date).AddSeconds(60)
    while ((Get-Date) -lt $deadline) {
        if ((Test-Path $pdf) -and ((Get-Item $pdf).LastWriteTime -gt $before) -and ((Get-Item $pdf).Length -gt 0)) { break }
        Start-Sleep -Milliseconds 500
    }
    Start-Sleep -Seconds 2
    if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
    if (-not ((Test-Path $pdf) -and ((Get-Item $pdf).LastWriteTime -gt $before))) { throw "Edge did not generate $pdf" }
    $item = Get-Item $pdf
    "Done: $($item.FullName) ($([math]::Round($item.Length / 1KB)) KB)"
}
