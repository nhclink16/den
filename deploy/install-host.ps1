param([Parameter(Mandatory=$true)][string]$Code)
$ErrorActionPreference = 'Stop'
if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') { throw 'This installer supports Windows x86_64.' }
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$release = Invoke-RestMethod 'https://api.github.com/repos/nhclink16/den/releases/latest'
$tag = $release.tag_name
if ($tag -notmatch '^v[0-9][A-Za-z0-9.-]*$') { throw 'Invalid release tag.' }
$base = "https://github.com/nhclink16/den/releases/download/$tag"
$asset = 'den-host-windows-x86_64.exe'
$temp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $temp | Out-Null
try {
    $download = Join-Path $temp 'den-host.exe'
    Invoke-WebRequest -UseBasicParsing "$base/$asset" -OutFile $download
    $sums = (Invoke-WebRequest -UseBasicParsing "$base/SHA256SUMS").Content
    $entries = @($sums -split "`n" | Where-Object { $_ -match "^[a-fA-F0-9]{64}\s+$([regex]::Escape($asset))\s*$" })
    if ($entries.Count -ne 1) { throw 'Missing or duplicate checksum. Nothing was installed.' }
    $expected = ($entries[0] -split '\s+')[0]
    if ((Get-FileHash $download -Algorithm SHA256).Hash -ne $expected) { throw 'Checksum mismatch. Nothing was installed.' }
    $directory = Join-Path $env:LOCALAPPDATA 'den'
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
    $exe = Join-Path $directory 'den-host.exe'
    # A running Windows executable cannot be replaced. Stop only our installed host.
    Get-Process -Name den-host -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $exe } | Stop-Process
    Copy-Item $download $exe -Force
    & $exe login $Code
    if ($LASTEXITCODE -ne 0) { throw 'Enrollment failed. Generate a new code in Settings, Machines.' }
    & $exe install
    if ($LASTEXITCODE -ne 0) { throw 'Service installation failed.' }
    Write-Output "Installed Den Host $tag. The machine will appear in Settings, Machines."
} finally {
    Remove-Item -Recurse -Force $temp
}
