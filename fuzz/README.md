# Fuzzing (cargo-fuzz)

Fuzzing de `parser::parse_chat_txt` y `grouping::group_novedades` con
[libFuzzer](https://llvm.org/docs/LibFuzzer.html).

## Requisitos

- Toolchain **nightly** (`cargo +nightly`): libFuzzer usa `-Zsanitizer=address`.
- `cargo-fuzz`:

```bash
cargo install cargo-fuzz --locked
```

## Ejecutar

```bash
cargo +nightly fuzz run parse_chat_txt
cargo +nightly fuzz run group_novedades
```

Con límite de tiempo:

```bash
cargo +nightly fuzz run parse_chat_txt -- -max_total_time=120
```

### Windows / sin enlazador MSVC

En Windows, libFuzzer requiere MSVC (Visual Studio Build Tools). Si no lo
tiene, ejecute el fuzzing dentro de Docker:

```bash
docker build -t parte-diario-fuzz fuzz/
docker run --rm -v "$PWD:/src" -w /src parte-diario-fuzz cargo fuzz run parse_chat_txt -- -max_total_time=120
```

El `Dockerfile` instala nightly + `cargo-fuzz`; la primera construcción tarda
unos minutos y luego queda en caché.

Los fallos (crashes) se guardan en `fuzz/artifacts/<target>/`. Reproducir uno:

```bash
cargo +nightly fuzz run parse_chat_txt fuzz/artifacts/parse_chat_txt/crash-...
```

## Corpus

Las semillas iniciales están en `fuzz/corpus/<target>/` (export de WhatsApp en
formato Android y iOS). libFuzzer añade ahí nuevos casos que amplían cobertura.

## CI

El workflow [`.github/workflows/fuzz.yml`](../.github/workflows/fuzz.yml) se
ejecuta a diario (03:00 UTC) y bajo demanda, con 5 minutos por target. Si hay
un crash, sube `fuzz/artifacts/` como artefacto del workflow.
