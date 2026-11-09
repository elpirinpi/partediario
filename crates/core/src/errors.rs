//! Error común de las librerías del workspace.

use thiserror::Error;

/// Error tipado para las capas de librería. La interfaz puede convertirlo a
/// un mensaje comprensible en español con [`AppError::user_message`].
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error de configuración: {0}")]
    Config(String),
    #[error("Error de seguridad: {0}")]
    Security(String),
    #[error("Error de red: {0}")]
    Network(String),
    #[error("Error del documento: {0}")]
    Document(String),
    #[error("Error de validación: {0}")]
    Validation(String),
    #[error("{0}")]
    Message(String),
}

impl AppError {
    /// Mensaje breve y accionable para mostrar en la interfaz.
    pub fn user_message(&self) -> String {
        match self {
            Self::Io(e) => format!("No se pudo acceder a un archivo: {e}"),
            Self::Config(m) => format!("Problema con la configuración: {m}"),
            Self::Security(m) => format!("Problema de seguridad: {m}"),
            Self::Network(m) => format!("No se pudo conectar con el servicio: {m}"),
            Self::Document(m) => format!("Problema con el documento: {m}"),
            Self::Validation(m) | Self::Message(m) => m.clone(),
        }
    }
}

/// Alias usado por las librerías del workspace.
pub type Result<T> = std::result::Result<T, AppError>;
