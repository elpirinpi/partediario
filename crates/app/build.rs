//! Recursos de Windows: incrusta el icono .ico en el .exe para que se vea
//! en el Explorador y en los accesos directos.
//!
//! Nota (toolchain gnu): enlazar vía `rustc-link-lib=static=resource` NO
//! funciona porque GNU ld solo extrae miembros de bibliotecas estáticas que
//! resuelvan símbolos, y un objeto de recursos (.rsrc) no tiene símbolos:
//! el linker lo descarta y el .exe queda sin icono. La solución correcta es
//! pasar el .o del recurso DIRECTAMENTE en la línea del linker.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icono-parte-diario.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let ico = std::path::Path::new("../../assets/icono-parte-diario.ico")
        .canonicalize()
        .expect("no se encontró assets/icono-parte-diario.ico");
    let rc_path = std::path::Path::new(&out_dir).join("resource.rc");
    let o_path = std::path::Path::new(&out_dir).join("resource.o");
    let version = env!("CARGO_PKG_VERSION");
    // code_page 65001 (UTF-8) para que el FileDescription con acentos sea
    // válido; la ruta del .ico va en comillas dobles con escapes de Windows.
    let rc = format!(
        "#pragma code_page(65001)\n\
         1 VERSIONINFO\n\
         FILEFLAGSMASK 0x3f\n\
         FILEVERSION 2,0,0,0\n\
         PRODUCTVERSION 2,0,0,0\n\
         FILETYPE 0x1\n\
         FILESUBTYPE 0x0\n\
         FILEOS 0x40004\n\
         FILEFLAGS 0x0\n\
         {{\n\
         BLOCK \"StringFileInfo\"\n\
         {{\n\
         BLOCK \"000004b0\"\n\
         {{\n\
         VALUE \"FileDescription\", \"Parte Diario - Anti-Ruidos y Contaminacion Sonica\\0\"\n\
         VALUE \"ProductVersion\", \"{version}\\0\"\n\
         VALUE \"ProductName\", \"Parte Diario\\0\"\n\
         VALUE \"FileVersion\", \"{version}\\0\"\n\
         }}\n\
         }}\n\
         BLOCK \"VarFileInfo\" {{\n\
         VALUE \"Translation\", 0x0, 0x04b0\n\
         }}\n\
         }}\n\
         1 ICON \"{ico}\"\n",
        ico = ico.display().to_string().replace('\\', "\\\\"),
        version = version,
    );
    if let Err(e) = std::fs::write(&rc_path, rc) {
        eprintln!("aviso: no se pudo escribir resource.rc: {e}");
        return;
    }
    let status = std::process::Command::new("windres")
        .args(["-O", "coff", "-o"])
        .arg(&o_path)
        .arg(&rc_path)
        .status();
    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bin=parte-diario={}", o_path.display());
        }
        Ok(s) => eprintln!("aviso: windres terminó con {s}; el .exe irá sin icono"),
        Err(e) => eprintln!("aviso: no se encontró windres ({e}); el .exe irá sin icono"),
    }
}
