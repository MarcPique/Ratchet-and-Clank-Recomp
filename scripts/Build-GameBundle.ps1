param(
    [Parameter(Mandatory)][string]$GameExe,
    [Parameter(Mandatory)][string]$ExtractorExe,
    [Parameter(Mandatory)][string]$ShadersDirectory,
    [Parameter(Mandatory)][string]$LicenseFile,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem

$game = (Resolve-Path -LiteralPath $GameExe).Path
$extractor = (Resolve-Path -LiteralPath $ExtractorExe).Path
$shaders = (Resolve-Path -LiteralPath $ShadersDirectory).Path
$license = (Resolve-Path -LiteralPath $LicenseFile).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$folder = Join-Path $output 'Ratchet & Clank PC'
$zip = Join-Path $output 'Ratchet-Clank-PC-Windows.zip'
if ((Test-Path -LiteralPath $folder) -or (Test-Path -LiteralPath $zip)) {
    throw "El destino ya contiene un paquete. Usa una carpeta de salida nueva: $output"
}
New-Item -ItemType Directory -Path (Join-Path $folder 'assets\shaders') -Force | Out-Null
Copy-Item -LiteralPath $game -Destination (Join-Path $folder 'Ratchet & Clank.exe')
Copy-Item -LiteralPath $extractor -Destination (Join-Path $folder 'Extraer datos.exe')
Get-ChildItem -LiteralPath $shaders -File -Filter '*.wgsl' | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $folder 'assets\shaders')
}
Copy-Item -LiteralPath $license -Destination (Join-Path $folder 'LICENSE')
@'
Ratchet & Clank PC - Windows

Para instalar desde tu ISO, usa el instalador gráfico publicado en GitHub.
Este paquete no incluye datos del juego. El ejecutable abre con doble clic
cuando existe una carpeta Data, creada por el instalador, junto a él.

Se necesita una ISO propia de Ratchet & Clank USA, serial SCUS_971.99.
El juego es una compilación personal y no oficial.
'@ | Set-Content -LiteralPath (Join-Path $folder 'LEEME.txt') -Encoding UTF8

[IO.Compression.ZipFile]::CreateFromDirectory($folder, $zip, [IO.Compression.CompressionLevel]::Optimal, $true)
Write-Output $zip
