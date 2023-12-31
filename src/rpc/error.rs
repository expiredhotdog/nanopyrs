use crate::NanoError;
use serde_json as json;
use json::Error as JsonError;
use reqwest::Error as ReqwestError;
use thiserror::Error;
use std::num::ParseIntError;
use std::convert::From;
use hex::FromHexError;

#[derive(Debug, Error)]
pub enum RpcError {
    #[error(transparent)]
    ReqwestError(#[from] ReqwestError),
    #[error(transparent)]
    JsonError(#[from] JsonError),
    #[error("parsing error: {0}")]
    ParseError(String),
    #[error("data was invalid")]
    InvalidData,
    #[error("this action could not be completed")]
    CommandFailed,
    /// Cannot publish block of type `legacy`
    #[error("cannot publish block of type 'legacy'")]
    LegacyBlockType
}
impl RpcError {
    pub fn from_option<T>(option: Option<T>) -> Result<T, RpcError> {
        option.ok_or(
            RpcError::ParseError("Option<T> returned empty".into())
        )
    }
}
impl From<ParseIntError> for RpcError {
    fn from(value: ParseIntError) -> Self {
        RpcError::ParseError(value.to_string())
    }
}
impl From<NanoError> for RpcError {
    fn from(value: NanoError) -> Self {
        RpcError::ParseError(value.to_string())
    }
}
impl From<FromHexError> for RpcError {
    fn from(value: FromHexError) -> Self {
        RpcError::ParseError(value.to_string())
    }
}