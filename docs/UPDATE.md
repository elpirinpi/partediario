# Actualizaciones desde GitHub

La app comprueba **GitHub Releases** y se actualiza sola cuando hay una versión
nueva. No requiere token: el repositorio debe ser **público**.

## Cómo funciona

1. Consulta `https://api.github.com/repos/<usuario>/<repo>/releases/latest`.
2. Compara el tag (`vX.Y.Z`) con la versión compilada de la app.
3. Descarga el asset `.zip`, verifica su **SHA-256** contra el asset `.sha256`.
4. Extrae `parte-diario.exe` a una carpeta temporal.
5. Al reiniciar: mueve el `.exe` actual a `%APPDATA%\parte-diario\backups\` y
   copia el nuevo. Si algo falla, restaura el respaldo.
6. La app se reinicia con la versión nueva.

Si la comprobación al iniciar está activada y hay versión nueva, aparece una
banda en la parte superior con **Instalar y reiniciar**.

## Requisitos

- Repositorio **público** en GitHub, o un repositorio público solo para las
  releases si el código es privado (ver más abajo).
- Una release **publicada** (no borrador ni pre-release) con:
  - un asset `.zip` que contenga `parte-diario.exe` (en la raíz del ZIP);
  - un asset `<mismo-nombre>.zip.sha256` con el hash SHA-256.
- El workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml)
  genera ambos automáticamente al crear un tag `vX.Y.Z`.

## Si el repositorio de código es privado

El actualizador **no usa token**, así que no puede leer releases privadas. Para
mantener el código privado, publique las releases en un **repositorio público
aparte**:

1. Cree un repositorio público, por ejemplo `elpirinpi/partediario-releases`,
   con un commit inicial (un `README.md`). No hace falta código; solo alojará
   las releases.
2. Cree un **fine-grained PAT** con permiso `Contents: Read and write` sobre ese
   repositorio público (y `Metadata: Read`, que va incluido).
3. En el repositorio **privado**: Settings → Secrets and variables → Actions:
   - **Secret** `RELEASE_TOKEN` = el PAT.
   - **Variable** `UPDATE_REPOSITORY` = `elpirinpi/partediario-releases`.
4. Empuje el tag `vX.Y.Z`. El workflow compila en el repo privado, publica los
   assets en el repo público y compila el binario apuntando a ese repo público.
5. El repositorio de código puede seguir privado: el actualizador solo consulta
   el repo público de releases.

> Configure `UPDATE_REPOSITORY` y `RELEASE_TOKEN` **antes** de empujar el tag,
> porque el binario embebe el repositorio en tiempo de compilación.

Si prefiere no crear un repo aparte, haga **público** el repositorio principal y
omita `UPDATE_REPOSITORY`/`RELEASE_TOKEN`: el workflow publicará en el mismo repo.

## Puesta en marcha (primera vez)

1. Cree un repositorio público en GitHub, por ejemplo `elpirinpi/partediario`.

2. Suba el proyecto:

   ```bash
   cd parte-diario
   git init
   git add .
   git commit -m "Parte Diario v2.0.0"
   git branch -M main
   git remote add origin https://github.com/elpirinpi/partediario.git
   git push -u origin main
   ```

3. En la app: **⚙ Ajustes → Actualizaciones → Repositorio de GitHub**, escriba
   `elpirinpi/partediario`, marque **Buscar actualizaciones al iniciar** y pulse
   **Guardar**.

> Si actualiza el campo `repository` del `Cargo.toml` del workspace con la URL
> de su repositorio, las instalaciones nuevas detectan el repositorio solas;
> también puede escribirlo a mano en Ajustes.

### Repositorio embebido en las releases

El workflow de release compila con
`PARTE_DIARIO_UPDATE_REPOSITORY=${{ github.repository }}`, por lo que el binario
publicado ya apunta a tu repositorio. Las instalaciones nuevas no necesitan
configurar nada.

Para una compilación local que también quede enlazada:

```powershell
$env:PARTE_DIARIO_UPDATE_REPOSITORY = "elpirinpi/partediario"
cargo build --release -p parte-diario-app --bin parte-diario
```

Si no se define, se usa el `repository` del `Cargo.toml`; si tampoco es válido,
la app pide el repositorio en Ajustes.

4. Publique la primera release. Para que la app detecte una actualización, la
   versión debe ser **mayor** que la instalada. Por ejemplo, para pasar de
   2.0.0 a 2.0.1:

   ```bash
   # editar Cargo.toml: [workspace.package] version = "2.0.1"
   cargo check                 # actualiza Cargo.lock
   git add Cargo.toml Cargo.lock
   git commit -m "chore: versión 2.0.1"
   git push
   git tag v2.0.1
   git push origin v2.0.1
   ```

   El tag dispara el workflow y publica:
   - `parte-diario-windows-x64.zip` + `.sha256`
   - `ParteDiario-2.0.1-x64.msi` + `.sha256`
   - `parte-diario.exe` + `.sha256`

5. Abra la app 2.0.0: al iniciar verá el aviso **⬆ Nueva versión v2.0.1
   disponible**. Pulse **Instalar y reiniciar**.

> La primera release `v2.0.0` no se considera actualización para una app 2.0.0.
> Hace falta publicar una versión superior.

## Instalación manual (MSI)

El MSI no lo usa el actualizador; sirve para instalar en un equipo nuevo:

`ParteDiario-X.Y.Z-x64.msi` desde la página de Releases.

## Reverso (rollback)

Si una actualización falla, en **Ajustes → Actualizaciones → Restaurar
respaldo** la app vuelve al `.exe` anterior y se reinicia. Los respaldos están en:

- Windows: `%APPDATA%\parte-diario\backups\`
- Linux/macOS: `~/.config/parte-diario/backups/`

## Problemas frecuentes

| Mensaje | Causa / solución |
|---------|------------------|
| `Falta configurar el repositorio…` | Escriba `usuario/repositorio` en Ajustes y guarde. |
| `GitHub respondió 404` | Repositorio mal escrito o privado (debe ser público). |
| `No hay ninguna versión publicada` | Aún no existe una release publicada (los borradores no cuentan). |
| `Ya tiene la última versión disponible` | El tag no es mayor que la versión instalada. |
| `El checksum no coincide` | Asset `.sha256` incorrecto o descarga corrupta. Vuelva a publicar la release. |
| `El .zip no contiene parte-diario.exe` | El ZIP no tiene el ejecutable en la raíz. |

## Límite de la API

La API pública de GitHub permite ~60 peticiones por hora sin autenticación. La
app comprueba una vez por arranque; si abre la app muchas veces al día y quiere
evitar el límite, desmarque la búsqueda al iniciar y use el botón manual de
Ajustes.
