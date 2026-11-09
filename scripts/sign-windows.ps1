# Firma Authenticode de archivos Windows con signtool.
#
# Requiere en el entorno:
#   WINDOWS_CERT_PFX_BASE64  Certificado .pfx codificado en base64
#   WINDOWS_CERT_PASSWORD    Contraseña del .pfx
#
# Uso:
#   ./scripts/sign-windows.ps1 archivo1.exe archivo2.msi
param(
    [Parameter(Mandatory = $true, ValueFromRemainingArguments = $true)]
    [string[]] $Files
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($env:WINDOWS_CERT_PFX_BASE64)) {
    throw "Falta la variable WINDOWS_CERT_PFX_BASE64"
}
if ([string]::IsNullOrWhiteSpace($env:WINDOWS_CERT_PASSWORD)) {
    throw "Falta la variable WINDOWS_CERT_PASSWORD"
}

$pfx = Join-Path ([IO.Path]::GetTempPath()) ("parte-diario-" + [guid]::NewGuid().ToString("N") + ".pfx")
[IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERT_PFX_BASE64))

try {
    $signtool = (Get-Command signtool.exe -ErrorAction SilentlyContinue).Source
    if (-not $signtool) {
        $kitsDirs = @(
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin",
            "$env:ProgramFiles\Windows Kits\10\bin"
        ) | Where-Object { Test-Path $_ }

        foreach ($kitsDir in $kitsDirs) {
            $signtool = Get-ChildItem $kitsDir -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
                Where-Object { $_.FullName -match '\\x64\\' } |
                Select-Object -First 1 -ExpandProperty FullName
            if ($signtool) { break }
        }
    }
    if (-not $signtool) {
        throw "No se encontró signtool.exe (instale el Windows SDK)"
    }

    foreach ($file in $Files) {
        if (-not (Test-Path $file)) {
            throw "No existe el archivo a firmar: $file"
        }
        & $signtool sign /fd SHA256 /f $pfx /p $env:WINDOWS_CERT_PASSWORD /tr http://timestamp.digicert.com /td SHA256 $file
        if ($LASTEXITCODE -ne 0) { throw "La firma falló para $file" }

        & $signtool verify /pa $file
        if ($LASTEXITCODE -ne 0) { throw "La verificación de firma falló para $file" }

        Write-Output "Firmado y verificado: $file"
    }
}
finally {
    Remove-Item $pfx -Force -ErrorAction SilentlyContinue
}
