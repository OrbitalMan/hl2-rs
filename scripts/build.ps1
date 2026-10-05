param([switch]$DebugBuild)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $cargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    $cargoBinary = if ($cargoCommand) { $cargoCommand.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    if (!(Test-Path -LiteralPath $cargoBinary)) { throw 'Install Rust using rustup first.' }
    $profileName = if ($DebugBuild) { 'debug' } else { 'release' }
    $cargoArguments = @('build', '--locked', '-p', 'hl2-runtime', '--bin', 'hl2-rs')
    if (!$DebugBuild) { $cargoArguments += '--release' }
    & $cargoBinary @cargoArguments
    if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed.' }
    New-Item -ItemType Directory -Force -Path 'bin' | Out-Null
    Copy-Item -LiteralPath "target\$profileName\hl2-rs.exe" -Destination 'bin\hl2-rs.exe'
    [ordered]@{
        built_utc = [DateTime]::UtcNow.ToString('o')
        profile = $profileName
        executable = 'bin/hl2-rs.exe'
        sha256 = (Get-FileHash -LiteralPath 'bin\hl2-rs.exe' -Algorithm SHA256).Hash
    } | ConvertTo-Json | Set-Content -LiteralPath 'bin\build-info.json'
    Write-Host 'Built bin\hl2-rs.exe. Run launch.cmd.'
} finally { Pop-Location }
