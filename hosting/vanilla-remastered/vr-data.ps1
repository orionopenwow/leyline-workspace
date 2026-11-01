# Vanilla Remastered data, the PC side (run by "Vanilla Remastered data.bat" and "... - check.bat").
#   powershell -NoProfile -ExecutionPolicy Bypass -File vr-data.ps1 [-Data "D:\Games\1.12\Data"] [-CheckOnly]
# 1. Finds this PC's 1.12.1 Data folder (the owner's own client) and its archives (the base ones, patch.MPQ and
#    patch-2..9.MPQ; never other servers' letter patches).
# 2. Asks the server whether there is disk for them and the job (vr-data.sh check). -CheckOnly stops here.
# 3. Uploads the archives to /srv/vr112/upload in 256 MB pieces: an interrupted upload goes on where it stopped,
#    each archive is checked with SHA-256 on both sides before it counts. Archives already there and checked are
#    skipped. SSH logins are spaced (the server pauses SSH after many quick logins).
# 4. Starts the server's job (vr-data.sh run: extract, Classic+ tables, slice, settings, delete the archives) and
#    follows its log. If the window or the connection drops, run the step again: it follows the same job.
# Uses the existing SSH key (.ssh\openwow), like every Leyline step. Prints no secret.
param([string]$Data = "", [switch]$CheckOnly)
$ErrorActionPreference = "Stop"
$Key = Join-Path $env:USERPROFILE ".ssh\openwow"
$HostName = "root@play.openvanilla.app"
$Kit = "/srv/vr112/pc-kit"
$Chunk = 256MB
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$script:LastSsh = [DateTime]::MinValue

function Say($m) { Write-Host $m }
function Fail($m) { Write-Host ""; Write-Host "VR DATA FAILED: $m"; exit 1 }
function SshArgs($cmd) { "-i `"$Key`" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30 -o ServerAliveCountMax=6 $HostName `"$cmd`"" }
function Pace() {   # at least 6 s between SSH logins
  $wait = 6 - ((Get-Date) - $script:LastSsh).TotalSeconds
  if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]($wait * 1000)) }
  $script:LastSsh = Get-Date
}
# One SSH command; stdin from a file range (binary-safe: written to the process's own stream, never through a
# PowerShell pipeline). Returns @{code; out}.
function Ssh($cmd, $inFile = $null, [long]$offset = 0, [long]$length = 0, [switch]$Show) {
  Pace
  $psi = New-Object System.Diagnostics.ProcessStartInfo
  $psi.FileName = "ssh"; $psi.Arguments = (SshArgs $cmd); $psi.UseShellExecute = $false
  $psi.RedirectStandardInput = $true; $psi.RedirectStandardOutput = (-not $Show)
  $p = [System.Diagnostics.Process]::Start($psi)
  if ($inFile) {
    $fs = [System.IO.File]::OpenRead($inFile)
    try {
      [void]$fs.Seek($offset, [System.IO.SeekOrigin]::Begin)
      $buf = New-Object byte[] (1MB); $left = $length; $in = $p.StandardInput.BaseStream
      while ($left -gt 0) {
        $n = $fs.Read($buf, 0, [int][Math]::Min($buf.Length, $left))
        if ($n -le 0) { break }
        $in.Write($buf, 0, $n); $left -= $n
      }
      $in.Flush()
    } finally { $fs.Close() }
  }
  $p.StandardInput.Close()
  $out = ""; if (-not $Show) { $out = $p.StandardOutput.ReadToEnd() }
  $p.WaitForExit()
  return @{ code = $p.ExitCode; out = $out }
}

if (-not (Test-Path $Key)) { Fail "SSH key $Key not found" }
if (-not (Get-Command ssh -ErrorAction SilentlyContinue)) { Fail "ssh.exe is missing (Windows' OpenSSH client)" }
if (-not (Get-Command tar -ErrorAction SilentlyContinue)) { Fail "tar.exe is missing (Windows 10 1803 or later has it)" }

# ------------------------------------------------------------------------------ 1. the Data folder
$cands = @()
if ($Data) { $cands += $Data }
$cands += @((Join-Path $env:USERPROFILE "Desktop\VanillaClassic\Data"), (Join-Path $env:USERPROFILE "Games\WoW-1.12.1\Data"), (Join-Path $env:USERPROFILE "Desktop\OpenVanilla\Data"),
            (Join-Path $env:USERPROFILE "Games\1.12.1\Data"), "C:\Games\1.12.1\Data")
$DataDir = $null
foreach ($c in $cands) { if ($c -and (Test-Path (Join-Path $c "dbc.MPQ"))) { $DataDir = $c; break } }
if (-not $DataDir) { Fail ("no 1.12.1 Data folder with dbc.MPQ found (looked in: " + ($cands -join "; ") + "). Give it as the step's first argument.") }
$files = Get-ChildItem -LiteralPath $DataDir -File | Where-Object {
  $_.Name -match '^(dbc|model|texture|terrain|wmo|interface|misc|fonts|sound|speech|patch|patch-[2-9])\.mpq$' }
$need = @("dbc","model","texture","terrain","wmo","interface","misc","fonts","sound","speech","patch")
foreach ($n in $need) { if (-not ($files | Where-Object { $_.BaseName -ieq $n })) { Fail "$DataDir has no $n.MPQ (not a full 1.12.1 client)" } }
$total = ($files | Measure-Object Length -Sum).Sum
$totalMB = [long][Math]::Ceiling($total / 1MB)
Say "Data folder: $DataDir"
Say ("Archives:    " + $files.Count + " (" + $totalMB + " MB): " + (($files | ForEach-Object { $_.Name }) -join ", "))

# ------------------------------------------------------------------------------ 2. the kit and the disk check
$tgz = Join-Path $env:TEMP "vr-data-kit.tgz"
if (Test-Path $tgz) { Remove-Item $tgz -Force }
Push-Location $Root
try {
  & tar -czf $tgz hosting/vanilla-remastered/vr-data.sh hosting/vanilla-remastered/vr-slice.py hosting/vanilla-remastered/slice-112.json hosting/vanilla-remastered/mpq-tools.Dockerfile
  if ($LASTEXITCODE -ne 0) { Fail "packing the kit failed" }
} finally { Pop-Location }
$len = (Get-Item $tgz).Length
if ($CheckOnly) {
  $r = Ssh "T=`$(mktemp -d) && tar -xzf - -C `$T && sed -i 's/\r$//' `$T/hosting/vanilla-remastered/*.sh && bash `$T/hosting/vanilla-remastered/vr-data.sh `$T check $totalMB; rc=`$?; rm -rf `$T; exit `$rc" $tgz 0 $len -Show
  Remove-Item $tgz -Force
  if ($r.code -ne 0) { exit 1 }
  Say ""; Say "Read-only check finished: nothing was uploaded or changed."
  exit 0
}
$r = Ssh "mkdir -p $Kit && rm -rf $Kit/hosting && tar -xzf - -C $Kit && sed -i 's/\r$//' $Kit/hosting/vanilla-remastered/*.sh && bash $Kit/hosting/vanilla-remastered/vr-data.sh $Kit check $totalMB" $tgz 0 $len -Show
Remove-Item $tgz -Force
if ($r.code -ne 0) { Fail "the server's check did not pass (see above); nothing was uploaded" }

# ------------------------------------------------------------------------------ 3. upload (resumable, checked)
$cacheFile = Join-Path $env:LOCALAPPDATA "Leyline\vr112-hashes.json"
$cache = @{}
if (Test-Path $cacheFile) { try { (Get-Content $cacheFile -Raw | ConvertFrom-Json).PSObject.Properties | ForEach-Object { $cache[$_.Name] = $_.Value } } catch {} }
function HashOf($f) {
  $k = $f.Name + "|" + $f.Length + "|" + $f.LastWriteTimeUtc.Ticks
  if ($cache.ContainsKey($k)) { return $cache[$k] }
  Say ("  checksum of " + $f.Name + " ...")
  $h = (Get-FileHash -LiteralPath $f.FullName -Algorithm SHA256).Hash.ToLower()
  $cache[$k] = $h
  New-Item -ItemType Directory -Force -Path (Split-Path $cacheFile) | Out-Null
  ($cache | ConvertTo-Json) | Set-Content -Path $cacheFile -Encoding ASCII
  return $h
}
$st = Ssh "bash $Kit/hosting/vanilla-remastered/vr-data.sh $Kit recv-status"
if ($st.code -ne 0) { Fail "could not read the upload folder on the server" }
$have = @{}; $parts = @{}
foreach ($line in ($st.out -split "`n")) {
  $p = $line.Trim() -split "`t"
  if ($p[0] -eq "file" -and $p.Count -ge 4) { $have[$p[1].ToLower()] = @{ size = [long]$p[2]; sha = $p[3] } }
  if ($p[0] -eq "part" -and $p.Count -ge 3) { $parts[$p[1].ToLower()] = [long]$p[2] }
}
$i = 0
foreach ($f in $files) {
  $i++
  $sha = HashOf $f
  $h = $have[$f.Name.ToLower()]
  if ($h -and $h.size -eq $f.Length -and $h.sha -eq $sha) { Say ("[" + $i + "/" + $files.Count + "] " + $f.Name + ": already on the server and checked"); continue }
  $off = 0
  if ($parts.ContainsKey($f.Name.ToLower())) { $off = [long]([Math]::Floor($parts[$f.Name.ToLower()] / $Chunk) * $Chunk) }
  if ($off -gt $f.Length) { $off = 0 }
  if ($off -gt 0) { Say ("[" + $i + "/" + $files.Count + "] " + $f.Name + ": going on from " + [long]($off / 1MB) + " MB") }
  do {
    $n = [long][Math]::Min($Chunk, $f.Length - $off)
    $last = ($off + $n) -ge $f.Length
    $cmd = "bash $Kit/hosting/vanilla-remastered/vr-data.sh $Kit recv-chunk $($f.Name) $off"
    if ($last) { $cmd += " && bash $Kit/hosting/vanilla-remastered/vr-data.sh $Kit recv-finish $($f.Name) $sha $($f.Length)" }
    $t0 = Get-Date
    $tries = 0
    while ($true) {
      $r = Ssh $cmd $f.FullName $off $n
      if ($r.code -eq 0) { break }
      $tries++
      if ($tries -ge 3) { Write-Host $r.out; Fail ("uploading " + $f.Name + " failed three times at " + [long]($off / 1MB) + " MB; run the step again to go on from there") }
      Say "  the piece did not arrive; trying again in 20 s"; Start-Sleep -Seconds 20
    }
    $secs = [Math]::Max(1, ((Get-Date) - $t0).TotalSeconds)
    $off += $n
    Say ("[" + $i + "/" + $files.Count + "] " + $f.Name + ": " + [long]($off / 1MB) + " / " + [long]($f.Length / 1MB) + " MB (" + [Math]::Round($n / 1MB / $secs, 1) + " MB/s)")
  } while ($off -lt $f.Length)
  if ($r.out -notmatch "CHECKED") { Write-Host $r.out; Fail ($f.Name + " did not check out on the server") }
}
Say ""; Say "All archives are on the server and checked."

# ------------------------------------------------------------------------------ 4. the job
$envs = ""
foreach ($v in @("VR_MMAPS", "VR_BOTS", "VR_BUDGET_MB", "VR_KEEP_UPLOAD")) {
  $val = [Environment]::GetEnvironmentVariable($v)
  if ($val -and $val -match '^[0-9]+$') { $envs += "$v=$val " }
}
$r = Ssh "$envs bash $Kit/hosting/vanilla-remastered/vr-data.sh $Kit run" -Show
if ($r.code -ne 0) { Fail "the server's job did not finish (see above). Run the step again: it goes on where it stopped." }
exit 0
