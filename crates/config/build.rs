fn main() {
    // Permite embeber el repositorio de actualizaciones en tiempo de compilación
    // (el workflow de release lo define con `github.repository`).
    println!("cargo:rerun-if-env-changed=PARTE_DIARIO_UPDATE_REPOSITORY");
}
