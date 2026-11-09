# Publicar una release

El workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml)
se dispara al crear un tag `vX.Y.Z` y publica en GitHub Releases:

| Artefacto | Uso |
|-----------|-----|
| `parte-diario-windows-x64.zip` (+ `.sha256`) | Auto-actualizador de la app |
| `ParteDiario-X.Y.Z-x64.msi` (+ `.sha256`) | Instalación manual en Windows |
| `parte-diario.exe` (+ `.sha256`) | Ejecutable portable |

## Pasos

1. Actualice la versión del workspace en `Cargo.toml` (`[workspace.package] version`).
2. Actualice `Cargo.lock` (por ejemplo con `cargo check`).
3. Ejecute la suite local y haga commit:

   ```bash
   cargo fmt --all -- --check
   cargo clippy --locked --workspace --all-targets -- -D warnings
   cargo test --locked --workspace
   git add .
   git commit -m "chore: versión X.Y.Z"
   git push
   ```

4. Cree y suba el tag:

   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

El workflow comprueba que el tag coincida con la versión de `parte-diario-app`
y falla si no es así.

> Si el repositorio de código es privado, configure `UPDATE_REPOSITORY` y
> `RELEASE_TOKEN` para publicar en un repo público de releases; consulte
> [UPDATE.md](UPDATE.md#si-el-repositorio-de-código-es-privado).

## Firma de código (opcional)

Configure estos secretos del repositorio (Settings → Secrets and variables →
Actions):

| Secreto | Contenido |
|---------|-----------|
| `WINDOWS_CERT_PFX_BASE64` | Certificado `.pfx` codificado en base64 |
| `WINDOWS_CERT_PASSWORD` | Contraseña del `.pfx` |

Si existen, el workflow:

1. Firma `target/release/parte-diario.exe` **antes** de construir el MSI.
2. Construye el MSI (que incrusta el ejecutable firmado).
3. Firma el MSI.
4. Verifica ambas firmas con `signtool verify /pa`.

Si no existen, la release se publica sin firmar y el workflow emite un aviso.

Para generar el base64 en PowerShell:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certificado.pfx")) |
  Set-Clipboard
```

## Requisitos del instalador

- `cargo-wix 0.3.9` y WiX Toolset 3.14.1. El runner `windows-latest` ya incluye
  WiX v3; si no estuviera, el workflow lo descarga.
- El MSI usa `crates/app/wix/main.wxs` y `License.rtf`.
- Instala en `C:\Program Files\Parte Diario\bin`, con acceso directo en el menú
  Inicio y sin modificar el PATH del sistema.

## Construir el MSI localmente (Windows)

```powershell
cargo install cargo-wix --locked --version 0.3.9
$env:PARTE_DIARIO_UPDATE_REPOSITORY = "elpirinpi/partediario"   # opcional pero recomendado
cargo build --release --locked -p parte-diario-app --bin parte-diario
cd crates/app
cargo wix -p parte-diario-app --no-build
```

El MSI queda en `target/wix/`.
