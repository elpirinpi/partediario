# Sistema de diseño — Concepto A "Esencial" (Parte Diario)

Documento de trabajo del rediseño (09-11-2026). El concepto A es el brief
y manda; esta nota fija lo que el brief deja libre, con criterio de
diseño propio, no de plantilla.

## La materia

Esto es la herramienta oficial del Parte Diario de un departamento de la
Policía Nacional (Dirección de Anti-Ruidos y Contaminación Sónica, P.N.).
Su usuario es el digitador que arma el parte cada noche desde el chat de
WhatsApp. El diseño debe sentirse como un documento oficial: institucional,
disciplinado y operativo. No es un dashboard SaaS ni una app de consumo.

## Color (6 hex con nombre)

| Nombre | Hex | Uso |
|---|---|---|
| Papel | `#F4F6FA` | Fondo de la app (gris azulado frío, nunca crema cálida) |
| Tarjeta | `#FFFFFF` | Superficies de trabajo; la hoja del documento también es blanca |
| Tinta | `#1A2433` | Texto principal (tinta, no negro puro) |
| Azul institucional | `#0F4C9C` | Acento: paso actual, enlaces, marca PD |
| Azul profundo | `#1B3A6B` | Tarjeta oscura "Turno de hoy" (el azul de la institución) |
| Dorado | `#C9A86A` | ÚNICAMENTE el botón "Generar Parte Diario" |

Secundarios (solo semántica, nunca decoración): texto secundario `#4A5568`,
atenuado `#7A8699`, borde sutil `#D5DCE8`, éxito `#1E7A4C`, aviso `#9A6B0F`,
error `#B3261E`. Sin degradados, sin sombras suaves, sin terracota.

## Tipo (una sola familia: Inter)

| Rol | Tamaño | Peso | Uso |
|---|---|---|---|
| Invitación | 22 | SemiBold | "Carga el chat de esta noche" (estado vacío) |
| Título | 16 | SemiBold | "Resumen", "Novedades (N)", "Novedad #N" |
| Sección | 14.5 | SemiBold | Títulos de sección dentro del editor |
| Cuerpo | 13.5 | Regular | Texto de trabajo, botones, filas del inspector |
| Pequeño | 12 | Regular | Filas de estado, pies, avisos |
| Leyenda | 11 | Regular | Etiquetas de campo, ayudas |

Reglas: formato oración en todas las etiquetas (nunca TODO EN MAYÚSCULAS);
los números solo aparecen en el stepper, que sí es una secuencia; nada de
monoespaciada para datos (el registro de actividad es la única excepción,
porque es un registro, no un dato).

## Layout (una frase + wireframe)

Tres columnas fijas como un expediente abierto: índice a la izquierda,
trabajo en el centro, resumen a la derecha. Todo alineado a la izquierda,
como un documento oficial; solo la invitación del estado vacío va centrada.

```
+----------+--------------------------------------+--------------+
| PD       |  ① Cargar  ② Analizar  ③ Extraer ...  | Resumen      |
| Parte    |                                        | Archivo      |
| Diario   |  Carga el chat de esta noche           | Fechas       |
|          |  [ . . . arrastra el .zip . . . ]      | Vista previa |
| Nuevo    |  [Cargar .zip]  [Ver ejemplo]          |              |
| Historial|                                        | [ Generar ]  |
| ...      |                                        |  dorado      |
| Turno    |                                        |              |
| de hoy   |                                        |              |
+----------+--------------------------------------+--------------+
| estado: 0 novedades, 0 incluidas, 0 fotos   v2.0.1  Registro |
+----------------------------------------------------------------+
```

## Principios (qué hace único este diseño)

1. **Es un expediente, no un dashboard.** La app es la versión digital del
   documento oficial: la columna central es el trabajo de la noche, el
   inspector derecho es el resumen del expediente y la vista previa muestra
   el documento real que se va a firmar. Nada de tarjetas de estadísticas
   ni gráficos decorativos.
2. **Disciplina institucional.** Un azul (P.N.), un dorado (la acción
   final) y todo lo demás callado. El atrevimiento vive en un solo lugar:
   la marca azul de la barra lateral y el botón dorado de Generar.
3. **El trabajo manda.** El centro siempre muestra la tarea actual
   (cargar, revisar, formalizar, generar); el stepper es la única
   secuencia numerada de la app. No hay decoración que no sirva al trabajo.
4. **Lenguaje del digitador.** Verbos activos, el mismo nombre de acción
   en todo el flujo ("Cargar" en el paso, en el botón y en la zona de
   arrastre), y el estado vacío como invitación a actuar, no como un
   letrero de "nada por aquí".

## Revisión contra el brief (qué cambié y por qué)

- **Papel cálido `#F8F6F1` en la vista previa → blanco `#FFFFFF`.** El
  crema cálido es el tic genérico más reconocible; el documento real es
  papel blanco y el fondo del brief ya es gris azulado frío.
- **Etiquetas en MAYÚSCULAS → formato oración.** El rótulo en mayúsculas
  con tracking es cromo de plantilla; un documento de la P.N. no grita
  sus etiquetas. ("Rango", "Vista previa", "Formalizar todas con IA").
- **"A · B · C" con puntos medios → comas.** La barra de estado decía
  "0 novedades · 0 incluidas · 0 fotos": el patrón exacto de la plantilla
  genérica. Ahora dice lo mismo con comas, como lenguaje de oficina.
- **"Vista previa — así quedará el documento" → "Vista previa del
  documento".** El patrón "PALABRA — fragmento" es otro tic de plantilla;
  el subtítulo ya explica el resto.
- **Tarjetas idénticas en todo el editor → secciones lisas para el
  trabajo de texto.** Seis tarjetas iguales apiladas eran el kit SaaS.
  Ahora las secciones de texto (encabezados, texto original, texto formal)
  son secciones lisas con título y regla; la tarjeta queda solo para
  unidades distintas (encabezado de la novedad, fotos, reasignación).
- **Botón sólido azul en el estado vacío → botón callado.** El azul
  sólido era un segundo elemento atrevido; el único botón con color
  fuerte es el dorado de Generar. "Cargar .zip de WhatsApp" ahora es
  relleno claro con borde y texto azul.
- **"→" y "✓" eliminados de la interfaz.** Ninguna fuente del stack los
  trae (ni Inter ni Ubuntu-Light): se veían como cajitas vacías □, un
  defecto que ya existía en el diseño anterior. Además, la "→" en botones
  es un tic de plantilla. Los rangos ahora se leen "08:00 a 08:00" (español
  llano) y el paso completado del stepper dibuja su marca con líneas en
  vez de depender de un glifo.
- **Icono 📂 decorativo del estado vacío → eliminado.** No servía al
  trabajo; la invitación es el encabezado y la zona de arrastre.
- **Versión "v2.0.1 · hash" fuera de la barra lateral → barra de estado.**
  La cabecera de la barra lateral tenía cuatro renglones de texto; la
  versión vive mejor callada abajo, junto al registro.
