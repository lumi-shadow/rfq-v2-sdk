
use thiserror::Error;

pub type Result<T> = std::result::Result<T, FillDecoderError>;

#[derive(Error, Debug)]
pub enum FillDecoderError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Error: {0}")]
    Other(String),
}

impl FillDecoderError {
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::Validation(msg.into())
    }

    pub fn other<S: Into<String>>(msg: S) -> Self {
        Self::Other(msg.into())
    }
}
