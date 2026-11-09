//! Núcleo compartido del Parte Diario.
//!
//! Contiene información de compilación, rutas de datos del usuario, errores
//! tipados, anonimización opcional y validación defensiva de archivos ZIP.

pub mod build_info;
pub mod errors;
pub mod paths;
pub mod privacy;
pub mod zip_safety;

pub use errors::{AppError, Result};
