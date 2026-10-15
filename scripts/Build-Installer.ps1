param(
    [Parameter(Mandatory)][string]$BundleZip,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$bundle = (Resolve-Path -LiteralPath $BundleZip).Path
$out = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $out | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $repo 'installer\payload') | Out-Null
Copy-Item -LiteralPath $bundle -Destination (Join-Path $repo 'installer\payload\GameBundle.zip') -Force
$publish = Join-Path $out 'publish'
dotnet publish (Join-Path $repo 'installer\RatchetClankInstaller.csproj') -c Release -o $publish
if ($LASTEXITCODE -ne 0) { throw 'No se pudo compilar el instalador.' }
$exe = Join-Path $publish 'Instalar-Ratchet-Clank.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Falta el instalador compilado: $exe" }
Copy-Item -LiteralPath $exe -Destination (Join-Path $out 'Instalar-Ratchet-Clank.exe') -Force
Write-Output (Join-Path $out 'Instalar-Ratchet-Clank.exe')
