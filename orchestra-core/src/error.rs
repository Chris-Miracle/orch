//! Error types for orchestra-core.

use std::path::PathBuf;

use thiserror::Error;

/// All errors that can arise from registry operations.
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Underlying I/O failure (file not found, permission denied, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML serialization error (write/save path).
    #[error("YAML serialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// YAML parse error on load — includes file path and line context from serde_yaml.
    #[error("failed to parse registry at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },

    /// `dirs::home_dir()` returned `None` — cannot locate `~/.orchestra/`.
    #[error("cannot determine home directory; set $HOME or equivalent")]
    HomeNotFound,

    /// The registry YAML file did not exist at the expected path.
    #[error("registry not found at {path}")]
    RegistryNotFound { path: PathBuf },
}
