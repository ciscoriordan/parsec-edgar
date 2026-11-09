/// Errors returned by this crate.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP {status} from {url}")]
    Status { status: u16, url: String },

    #[error("network error for {url}: {message}")]
    Network { url: String, message: String },

    #[error("invalid JSON from {url}: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("company not found: {0}")]
    CompanyNotFound(String),

    #[error("filing not found: {0}")]
    FilingNotFound(String),

    #[error("{0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;
