# Guía de uso — Parte Diario (Anti-Ruidos y Contaminación Sónica, P.N.)

Aplicación de escritorio en Rust que convierte un **.zip exportado de WhatsApp**
en el **Parte Diario** oficial en Word (.docx), usando su plantilla con membrete.

Compila a **un único ejecutable** (`.exe` en Windows, binario nativo en
Linux/Mac), sin Python, sin pip, sin runtimes que instalar.

---

## 1. Requisitos para compilar

Solo necesita el compilador de Rust (una vez):

1. Instale Rust desde **https://rustup.rs** (opción por defecto).
2. Reinicie la terminal / símbolo del sistema.
3. Verifique: `cargo --version`

## 2. Compilar

```bash
cd parte-diario
cargo build --release
```

El ejecutable queda en:

- Windows: `target\release\parte-diario.exe`
- Linux/Mac: `target/release/parte-diario`

La primera compilación descarga las dependencias y puede tardar varios
minutos; las siguientes son rápidas. No requiere pasos manuales adicionales.

> **Nota:** en Windows se recomienda compilar en Windows (el `.exe` resultante
> es nativo). En Linux/Mac el binario es nativo de cada sistema.

## 3. Flujo de uso (paso a paso)

### Paso 1 — Exportar el chat de WhatsApp
En el teléfono: abrir el grupo/chat → *Exportar chat* → **Adjuntar archivos
multimedia**. Obtendrá un `.zip` con un `.txt` y las fotos/videos.

### Paso 2 — Cargar el .zip
Botón **1. Cargar .zip WhatsApp**. La app:

- Calcula y muestra el **SHA-256** del `.zip`.
- Detecta automáticamente el formato (**corchete** Android / **guion** iOS).
- Agrupa los mensajes en **novedades** (fotos antes o después del texto se
  asocian a la nota más cercana; varias fotos seguidas van a la misma novedad).
- **Ignora** mensajes de sistema y **descarta los videos** del documento
  (conservando la novedad y su texto).
- **Advierte** si el `.zip` (o su contenido) **ya fue procesado antes** — esto
  evita regenerar por confusión de nombres de archivo repetidos.
- **Pre-selecciona** solo las novedades con fecha posterior a su último
  reporte (el `.zip` suele traer todo el historial acumulado).
- Infiere la **fecha del reporte desde el cuerpo de las notas** (no del
  timestamp de envío) y **señala** cualquier novedad con fecha atípica
  (posible error de dedo del agente).

### Paso 3 — Cargar la plantilla .docx
Botón **📄 Plantilla**. La app recuerda la última usada.
La plantilla debe contener el membrete, el logo y la tabla inicial
("A la:" / "Asunto en este:"). La app reemplaza:

- la fecha del encabezado (ej. "10 de octubre, 2026."),
- el rango del Asunto ("...desde las 08:00 horas del día 10-10-2026 hasta
  las 08:00 horas del día 11-10-2026."),

y **elimina las tablas de novedades antiguas** que traiga la plantilla:
el reporte final contiene **únicamente las novedades del .zip**,
con el formato de la plantilla (tabla de dos columnas, Tahoma 13,
encabezados y fotos a la izquierda, narrativa a la derecha).
La app le avisa cuántas tablas viejas eliminó.

### Paso 4 — Revisar las novedades (revisión asistida)
En la barra superior, el **indicador de pasos** (ZIP → Plantilla → Revisar →
Formalizar → Fechas → Generar) muestra en qué punto del flujo está: pasos
hechos en verde, el actual en dorado y los pendientes en gris.

En la lista izquierda: marque/desmarque con el checkbox qué novedades entran
al reporte. Al seleccionar una verá:

- encabezados institucionales (editables),
- texto crudo (editable),
- **texto formal** generado (siempre editable antes de generar),
- miniaturas de las fotos y videos omitidos.

Fotos **huérfanas** (sin texto, por mensaje borrado): use
*"Mover fotos a la novedad elegida"* para reasignarlas visualmente.

Puede **excluir** reportes de otras divisiones, chistes, links o trámites
administrativos simplemente desmarcándolos: nada se filtra sin que usted
lo vea.

### Paso 5 — Formalizar el texto
Por novedad: **✨ Formalizar con IA** (requiere API key, ver §5) o
**🧹 Limpieza sin conexión** (reglas seguras, sin internet).
También puede **formalizar todas** de una vez. El texto generado **siempre**
se muestra para su revisión y edición manual antes de generar el documento.

> La redacción formal **preserva exactamente** nombres, cédulas, placas,
> teléfonos, horas, montos y lugares. No inventa ni elimina datos.

### Paso 5.5 — Vista previa del documento (antes de generar)
En el panel central, cambie a **👁 Vista previa** para ver el Parte Diario
tal como quedará: fecha, rango del asunto y cada novedad incluida con sus
encabezados, fotos y texto formal (las que aún no se formalizaron llevan la
marca "SIN FORMALIZAR"). La vista previa se actualiza sola al editar
cualquier novedad; vuelva a **✏️ Editar** para seguir trabajando.

### Paso 6 — Generar
Ajuste el rango de fechas si hace falta y pulse **3. Generar Parte Diario**.
La app:

- inserta **una fila por novedad** (izquierda: encabezados + fotos apiladas;
  derecha: narrativa formal + cierre),
- incrusta las fotos calculando sus dimensiones reales en EMU,
- valida internamente el OOXML (XML bien formado, relaciones de imagen,
  `w14:paraId` únicos de 31 bits),
- guarda el `.docx` como `Parte_Diario_DD-MM-AAAA_DD-MM-AAAA.docx`.

Si ya existe un reporte para ese rango de fechas, la app **pide confirmación**
antes de reemplazarlo (pulse Generar una segunda vez para confirmar el
reemplazo).

Use **Abrir .docx generado** para revisarlo en Word.

## 4. Historial y control de procesamiento

Todo queda en la carpeta de configuración del usuario
(`~/.config/parte-diario/` en Linux, `%APPDATA%\parte-diario\` en Windows):

- `config.json`: API key, modelo, carpeta de salida, última plantilla.
- `history.json`: SHA-256 de cada `.zip`, fechas de reporte generadas,
  ruta del `.docx` y cantidad de novedades.

Esto permite detectar duplicados y pre-seleccionar solo lo nuevo en cada
carga.

## 5. Redacción con IA: OpenRouter (gratis) o Anthropic

La app puede reescribir las narrativas en español formal con IA, o limpiarlas
sin conexión si prefiere no usar IA.

### Opción recomendada: OpenRouter (gratis)

1. Cree su cuenta y su API key **gratis y sin tarjeta** en
   https://openrouter.ai/keys
2. En la app: **⚙ Ajustes** → proveedor **"OpenRouter (modelos gratis :free)"**
   → pegue su key → Guardar.

El modelo configurado por defecto es `google/gemma-4-31b-it:free` (gratuito y
sin razonamiento visible). Si algún día deja de estar disponible gratis,
pruebe otro de la lista en Ajustes:

- `google/gemma-4-26b-a4b-it:free`
- `nvidia/nemotron-3.5-lightning:free`

Evite modelos que muestran su cadena de pensamiento (`Thinking Process`): la
app la detecta y rechaza la respuesta, pero es mejor usar un modelo de
instrucciones como Gemma.

**Límites del plan gratuito:** ~20 peticiones por minuto y ~50 por día. El
botón "Formalizar TODAS" las procesa una por una con pausas para respetar
ese límite. Si lo alcanza, la app le avisa y puede intentarlo más tarde.
También existe "🧹 Limpieza a TODAS (sin conexión)": aplica la corrección
local a todas las novedades incluidas de una vez, sin gastar la API.

### Opción local: FreeLLMApi

Si tiene FreeLLMApi corriendo en `http://127.0.0.1:31415`, seleccione el
proveedor **"FreeLLMApi (Local)"** y el modelo `auto`. La app envía
`reasoning_effort: none` para que el modelo no escriba su cadena de
pensamiento dentro del informe. Si `auto` falla, pruebe `gemma-4-31b-it` o
`gemini-3.5-flash`.

### Opción alternativa: Anthropic (Claude, de pago)

Si tiene API key de Anthropic: en **⚙ Ajustes** cambie el proveedor a
"Anthropic (Claude, de pago)", pegue su key y el modelo
(`claude-sonnet-4-5`).

La key se guarda **localmente** en su equipo, nunca se hardcodea. Sin key,
la app funciona con el modo de limpieza sin conexión
("🧹 Limpieza sin conexión").

## 6. Solución de problemas

| Problema | Causa / solución |
|---|---|
| "No se encontró la fecha del encabezado" | Su plantilla usa otro formato de fecha; edítela a mano en el .docx generado. |
| Word pide "reparar" el documento | Repórtelo: la app valida el OOXML antes de escribir; revise que la plantilla sea un .docx válido. |
| Una foto no aparece | No estaba dentro del .zip o el archivo está corrupto; el texto se conserva igual. |
| El .zip no parsea bien | Verifique que sea "Exportar chat → Adjuntar archivos multimedia" y que el .txt esté dentro. |
| En Linux no abre el diálogo de archivos | Instale `zenity` (o `kdialog`): `sudo apt install zenity`. |

## 7. Estructura del proyecto

```
parte-diario/
├── Cargo.toml
└── src/
    ├── main.rs        # punto de entrada (ventana eframe)
    ├── parser.rs      # parseo del .txt (formatos corchete/guion)
    ├── grouping.rs    # novedades, fechas, huérfanas, inconsistencias
    ├── llm.rs         # formalización (Anthropic / sin conexión)
    ├── docx_editor.rs # edición quirúrgica OOXML + validación
    ├── history.rs     # config, SHA-256, historial JSON
    └── gui.rs         # interfaz egui
```

## 8. Notas técnicas

- El `.docx` se trata como contenedor zip; `word/document.xml` se edita por
  reemplazo dirigido con fusión de runs (`<w:t>` divididos).
- Cada `<w:p>` nuevo lleva `w14:paraId` único de 31 bits (`UUIDv4 & 0x7FFFFFFF`,
  8 dígitos hexadecimales).
- Imágenes: `<w:drawing>` + relación en `word/_rels/document.xml.rels` +
  `Override` en `[Content_Types].xml`; ancho 2.36", alto máximo 3.15".
- Los videos se excluyen del documento pero nunca rompen el parseo.
