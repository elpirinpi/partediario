# Imagen para ejecutar cargo-fuzz en Windows/macOS sin toolchain nightly nativo.
# Uso:
#   docker build -t parte-diario-fuzz fuzz/
#   docker run --rm -v "$PWD:/src" -w /src parte-diario-fuzz cargo fuzz run parse_chat_txt
FROM rustlang/rust:nightly
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang lld \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-fuzz --locked --version 0.13.2
WORKDIR /src
