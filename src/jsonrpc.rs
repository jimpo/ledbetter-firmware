use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, derive_more::Display, derive_more::Error)]
pub enum Error {
	#[display(fmt = "jsonrpc version field is not \"2.0\": {}", _0)]
	BadJsonrpcVersion(#[error(not(source))] String),
	ResponseHasBothResultAndError,
	ResponseHasNeitherResultNorError,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Request<'a> {
	pub jsonrpc: &'a str,
	pub id: u32,
	pub method: &'a str,
	pub params: &'a serde_json::value::RawValue,
}

impl<'a> Request<'a> {
	pub fn validate(&self) -> Result<(), Error> {
		if self.jsonrpc != "2.0" {
			return Err(Error::BadJsonrpcVersion(self.jsonrpc.to_string()));
		}
		Ok(())
	}
}

#[derive(Debug, Clone, Serialize)]
pub struct Response<'a> {
	pub jsonrpc: &'a str,
	pub id: u32,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<Value>,
}

impl<'a> Response<'a> {
	pub fn validate(&self) -> Result<(), Error> {
		if self.jsonrpc != "2.0" {
			return Err(Error::BadJsonrpcVersion(self.jsonrpc.to_string()));
		}
		if self.result.is_none() && self.error.is_none() {
			return Err(Error::ResponseHasBothResultAndError);
		}
		if self.result.is_none() && self.error.is_none() {
			return Err(Error::ResponseHasNeitherResultNorError);
		}
		Ok(())
	}
}