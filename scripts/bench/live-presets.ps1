# Live per-preset reading: each preset borderless-fullscreen, 30 s, with music.
# Usage: .\scripts\bench\live-presets.ps1 [-Gpus NVIDIA,AMD]
# Fullscreen comes from [output] fullscreen in config.toml, which this script sets to true for
# the run and restores afterwards. Keep the window in front: an occluded window throttles.
param([string[]]$Gpus = @("NVIDIA", "AMD"))
# Invariant culture: a comma-decimal locale would otherwise print 5,68 into the tab-separated output.
[System.Threading.Thread]::CurrentThread.CurrentCulture = [cultureinfo]::InvariantCulture
Set-Location (git rev-parse --show-toplevel)
$Bin = Resolve-Path ".\target\release\ritmolux.exe"
$Dir = Join-Path $env:APPDATA "Ritmolux"
$Log = Join-Path $Dir "diagnostics.log"
$Cfg = Join-Path $Dir "config.toml"
$Presets = "Nebula","Leviathan","Clifford","Volute","Dragon","Ink on Paper","Barnsley Fern","Braid","Murmuration","Shatter"

$cfgBackup = Get-Content $Cfg -Raw
try {
  (Get-Content $Cfg -Raw) -replace '(?m)^fullscreen\s*=\s*false', 'fullscreen = true' | Set-Content $Cfg -NoNewline
  "gpu`tpreset`tsamples`tfps_med`tfps_min`tavg_ms_med`tp99_ms_med`tp99_ms_max`tdropped`tsamples_under_60"
  foreach ($gpu in $Gpus) {
    foreach ($p in $Presets) {
      $before = (Get-Content $Log).Count
      $proc = Start-Process -FilePath $Bin -ArgumentList "--gpu", "`"$gpu`"", "--preset", "`"$p`"" -PassThru
      Start-Sleep -Seconds 32
      Stop-Process -Id $proc.Id -Force; $proc.WaitForExit(); Start-Sleep -Seconds 1
      $new = Get-Content $Log | Select-Object -Skip $before
      $new | Where-Object { $_ -like '# renderer*' } | ForEach-Object { Write-Host $_ }
      # The first 6 one-second rows cover startup; they are dropped.
      $rows = @($new | Where-Object { $_ -notlike '#*' } | Select-Object -Skip 6 | ForEach-Object { ,($_ -split "`t") })
      if ($rows.Count -eq 0) { "$gpu`t$p`t0"; continue }
      $fps = $rows | ForEach-Object { [double]$_[1] } | Sort-Object
      $avg = $rows | ForEach-Object { [double]$_[2] } | Sort-Object
      $p99 = $rows | ForEach-Object { [double]$_[3] } | Sort-Object
      $n = $rows.Count; $m = [int][math]::Floor($n / 2)
      $dropped = [int]$rows[-1][5] - [int]$rows[0][5]
      $under = @($fps | Where-Object { $_ -lt 60 }).Count
      "{0}`t{1}`t{2}`t{3:N1}`t{4:N1}`t{5:N2}`t{6:N2}`t{7:N2}`t{8}`t{9}" -f $gpu, $p, $n, $fps[$m], $fps[0], $avg[$m], $p99[$m], $p99[-1], $dropped, $under
    }
  }
} finally {
  Set-Content $Cfg $cfgBackup -NoNewline
}
