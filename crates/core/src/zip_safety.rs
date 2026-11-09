//! Validación defensiva de ZIP: evita zip-slip, zip bombs y archivos enormes.

use std::io::{Read, Seek};

/// Límite de entradas para un export de WhatsApp razonable.
pub const MAX_ENTRIES: usize = 20_000;
/// Tamaño total descomprimido permitido (1.5 GB).
pub const MAX_TOTAL_UNCOMPRESSED: u64 = 1_500_000_000;
/// Tamaño máximo de un archivo individual (200 MB).
pub const MAX_FILE_UNCOMPRESSED: u64 = 200_000_000;
/// Relación máxima comprimido/descomprimido para archivos grandes.
pub const MAX_RATIO: u64 = 1_000;

/// Valida el índice del ZIP sin extraerlo. `enclosed_name` ya bloquea rutas
/// absolutas y `..`, pero además se controlan tamaños y ratios de compresión.
pub fn validate_zip<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<(), String> {
    if zip.len() > MAX_ENTRIES {
        return Err(format!(
            "El .zip contiene demasiados archivos ({}); el máximo permitido es {}.",
            zip.len(),
            MAX_ENTRIES
        ));
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let f = zip
            .by_index(i)
            .map_err(|e| format!("No se pudo leer la entrada {i} del .zip: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let name = f
            .name()
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_else(|_| format!("<entrada {i}>"));
        if f.enclosed_name().is_none() {
            return Err(format!(
                "El .zip contiene una ruta insegura: '{name}'. Se canceló la carga."
            ));
        }
        let size = f.size();
        if size > MAX_FILE_UNCOMPRESSED {
            return Err(format!(
                "El archivo '{name}' es demasiado grande ({} MB).",
                size / 1_000_000
            ));
        }
        if size > 10_000_000 {
            let compressed = f.compressed_size().max(1);
            if size / compressed > MAX_RATIO {
                return Err(format!(
                    "El archivo '{name}' parece un zip bomb (compresión anómala)."
                ));
            }
        }
        total = total.saturating_add(size);
        if total > MAX_TOTAL_UNCOMPRESSED {
            return Err(format!(
                "El .zip descomprimido supera el límite de {} MB.",
                MAX_TOTAL_UNCOMPRESSED / 1_000_000
            ));
        }
    }
    Ok(())
}
