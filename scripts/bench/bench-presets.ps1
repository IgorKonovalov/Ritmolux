# Per-preset headless frame cost: no vsync, no present. Usage: .\bench-presets.ps1 [-Gpu NVIDIA] [-Size 1920x1080]
param([string]$Gpu = "NVIDIA", [string]$Size = "1920x1080")
# Invariant culture: a comma-decimal locale would otherwise print 5,68 into the tab-separated output.
[System.Threading.Thread]::CurrentThread.CurrentCulture = [cultureinfo]::InvariantCulture
$Bin = ".\target\release\ritmolux.exe"
$Presets = "Nebula","Leviathan","Clifford","Volute","Dragon","Ink on Paper","Barnsley Fern","Braid","Murmuration","Shatter"
$err = Join-Path $env:TEMP "rlx-bench-stderr.txt"
function Run-One($p, $frames) {
  # cmd's >NUL discards the raw frames; PowerShell's own pipe would buffer them all.
  cmd /c "`"$Bin`" --stream --sink stdout --gpu `"$Gpu`" --size $Size --fps 240 --frames $frames --preset `"$p`" >NUL 2>`"$err`""
  Get-Content $err
}
Run-One "Nebula" 1 | Select-String '^renderer'
"preset`trun1`trun2`trun3`tmedian_ms`tfps_equiv"
foreach ($p in $Presets) {
  $r = 1..3 | ForEach-Object {
    $line = Run-One $p 1440 | Select-String 'draw\+submit ([0-9.]+) ms' | Select-Object -Last 1
    [double]$line.Matches[0].Groups[1].Value
  }
  $m = ($r | Sort-Object)[1]
  "{0}`t{1}`t{2}`t{3}`t{4}`t{5:N0}" -f $p, $r[0], $r[1], $r[2], $m, (1000 / $m)
}
