//! Versión, commit y fecha de compilación expuestos por la aplicación.

/// Versión semántica del workspace (`Cargo.toml`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Hash corto del commit con el que se compiló (o `dev`).
pub const GIT_HASH: &str = env!("PARTE_DIARIO_GIT_HASH");
/// Fecha UTC de compilación (`YYYY-MM-DD`).
pub const BUILD_DATE: &str = env!("PARTE_DIARIO_BUILD_DATE");

/// Línea lista para mostrar en la interfaz o en los logs.
pub fn version_line() -> String {
    format!("Parte Diario v{VERSION} ({GIT_HASH}, {BUILD_DATE})")
}
