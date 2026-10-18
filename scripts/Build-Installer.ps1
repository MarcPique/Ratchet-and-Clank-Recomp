param(
    [Parameter(Mandatory)][string]$BundleZip,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$Dlss5SourceDirectory
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$bundle = (Resolve-Path -LiteralPath $BundleZip).Path
$out = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $out | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $repo 'installer\payload') | Out-Null
Copy-Item -LiteralPath $bundle -Destination (Join-Path $repo 'installer\payload\GameBundle.zip') -Force

$runtimeZip = Join-Path $repo 'installer\payload\Dlss5Runtime.zip'
if ($Dlss5SourceDirectory) {
    $source = (Resolve-Path -LiteralPath $Dlss5SourceDirectory).Path
    $names = @('nvngx_dlssnr.dll', 'nvngx_dlss.dll', 'renodx-dlss5.addon64')
    $expected = @{
        'nvngx_dlssnr.dll' = '6EB209E764F39872625DEBD6ABAF45E2BB6322F6F270F781F70C059AE30B3927'
        'nvngx_dlss.dll' = '3975567B8943C53ACCE397F2B72380092F84F162D00B0D2C7D08A1025C563983'
        'renodx-dlss5.addon64' = 'DCD93881E976AD033D83C2BB01F4BC3E4DDC59C15FE0DD4CA165BC5FC7D1AC68'
    }
    foreach ($name in $names) {
        $component = Join-Path $source $name
        if (-not (Test-Path -LiteralPath $component -PathType Leaf)) {
            throw "Falta el componente DLSS 5: $name"
        }
        if ((Get-FileHash -LiteralPath $component -Algorithm SHA256).Hash -ne $expected[$name]) {
            throw "El componente $name no coincide con la instalación probada."
        }
    }
    Add-Type -AssemblyName System.IO.Compression
    if (Test-Path -LiteralPath $runtimeZip) { Remove-Item -LiteralPath $runtimeZip -Force }
    $archive = [IO.Compression.ZipFile]::Open($runtimeZip, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $names) {
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, (Join-Path $source $name), $name, [IO.Compression.CompressionLevel]::Optimal) | Out-Null
        }
    } finally { $archive.Dispose() }
} elseif (Test-Path -LiteralPath $runtimeZip) {
    Remove-Item -LiteralPath $runtimeZip -Force
}
$publish = Join-Path $out 'publish'
dotnet publish (Join-Path $repo 'installer\RatchetClankInstaller.csproj') -c Release -o $publish
if ($LASTEXITCODE -ne 0) { throw 'No se pudo compilar el instalador.' }
$exe = Join-Path $publish 'Instalar-Ratchet-Clank.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Falta el instalador compilado: $exe" }
Copy-Item -LiteralPath $exe -Destination (Join-Path $out 'Instalar-Ratchet-Clank.exe') -Force
Write-Output (Join-Path $out 'Instalar-Ratchet-Clank.exe')
