use std::borrow::Cow;
use serde::{Deserialize, Serialize, Deserializer};
use serde_json::value::{RawValue};

#[derive(Debug, Clone, derive_more::Display, derive_more::Error)]
pub enum Error {
	#[display(fmt = "jsonrpc version field is not \"2.0\": {}", _0)]
	BadJsonrpcVersion(#[error(not(source))] String),
	ResponseHasBothResultAndError,
	ResponseHasNeitherResultNorError,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Request<'a> {
	pub jsonrpc: &'a str,
	pub id: Cow<'a, RawValue>,
	pub method: &'a str,
	pub params: Cow<'a, RawValue>,
}

impl<'a> Request<'a> {
	pub fn validate(&self) -> Result<(), Error> {
		if self.jsonrpc != "2.0" {
			return Err(Error::BadJsonrpcVersion(self.jsonrpc.to_string()));
		}
		Ok(())
	}
}

fn deserialize_optional_value<'de, D>(deserializer: D)
	-> Result<Option<Cow<'de, RawValue>>, D::Error>
	where D: Deserializer<'de>
{
	let raw_value = <&'de RawValue>::deserialize(deserializer)?;
	Ok(Some(Cow::Borrowed(raw_value)))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Response<'a> {
	pub jsonrpc: &'a str,
	pub id: Cow<'a, RawValue>,
	#[serde(default)]
	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(deserialize_with = "deserialize_optional_value")]
	pub result: Option<Cow<'a, RawValue>>,
	#[serde(default)]
	#[serde(skip_serializing_if = "Option::is_none")]
	#[serde(deserialize_with = "deserialize_optional_value")]
	pub error: Option<Cow<'a, RawValue>>,
}

impl<'a> Response<'a> {
	pub fn validate(&self) -> Result<(), Error> {
		if self.jsonrpc != "2.0" {
			return Err(Error::BadJsonrpcVersion(self.jsonrpc.to_string()));
		}
		if self.result.is_some() && self.error.is_some() {
			return Err(Error::ResponseHasBothResultAndError);
		}
		if self.result.is_none() && self.error.is_none() {
			return Err(Error::ResponseHasNeitherResultNorError);
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use assert_matches::assert_matches;
	use serde_json::value::to_raw_value;

	#[test]
	fn test_deserialize_request() {
		let req_str = "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"add\",\"params\":[1,\"2\",null]}";
		let req: Request = serde_json::from_str(req_str).unwrap();
		req.validate().unwrap();
		assert_eq!(req.id.get(), "0");
		assert_eq!(req.method, "add");
		assert_eq!(req.params.get(), "[1,\"2\",null]");
	}

	#[test]
	fn test_deserialize_invalid_request_jsonrpc_version() {
		let req_str = "{\"jsonrpc\":\"3.0\",\"id\":0,\"method\":\"add\",\"params\":[1,\"2\",null]}";
		let req: Request = serde_json::from_str(req_str).unwrap();
		assert_matches!(req.validate(), Err(Error::BadJsonrpcVersion(_)));
	}

	#[test]
	fn test_deserialize_non_error_response() {
		let resp_str = "{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":3}";
		let resp: Response = serde_json::from_str(resp_str).unwrap();
		resp.validate().unwrap();
		assert_eq!(resp.id.get(), "0");
		assert_matches!(resp.result, Some(v) if v.get() == "3");
		assert!(resp.error.is_none());
	}

	#[test]
	fn test_deserialize_error_response() {
		let resp_str = "{\"jsonrpc\":\"2.0\",\"id\":0,\"error\":\"null arg\"}";
		let resp: Response = serde_json::from_str(resp_str).unwrap();
		resp.validate().unwrap();
		assert_eq!(resp.id.get(), "0");
		assert!(resp.result.is_none());
		assert_matches!(resp.error, Some(v) if v.get() == "\"null arg\"");
	}

	#[test]
	fn test_deserialize_invalid_response_with_result_and_error() {
		let resp_str = "{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":3,\"error\":\"null arg\"}";
		let resp: Response = serde_json::from_str(resp_str).unwrap();
		assert_matches!(resp.validate(), Err(Error::ResponseHasBothResultAndError));
	}

	#[test]
	fn test_deserialize_invalid_response_with_neither_result_nor_error() {
		let resp_str = "{\"jsonrpc\":\"2.0\",\"id\":0}";
		let resp: Response = serde_json::from_str(resp_str).unwrap();
		assert_matches!(resp.validate(), Err(Error::ResponseHasNeitherResultNorError));
	}

	#[test]
	fn test_deserialize_invalid_response_jsonrpc_version() {
		let resp_str = "{\"jsonrpc\":\"1.0\",\"id\":0,\"result\":3}";
		let resp: Response = serde_json::from_str(resp_str).unwrap();
		assert_matches!(resp.validate(), Err(Error::BadJsonrpcVersion(_)));
	}

	#[test]
	fn test_serialize_non_error_response() {
		let resp = Response {
			jsonrpc: "2.0",
			id: Cow::Owned(to_raw_value(&0).unwrap()),
			result: Some(Cow::Owned(to_raw_value(&3).unwrap())),
			error: None,
		};
		let expected_str = "{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":3}";
		let resp_str = serde_json::to_string(&resp).unwrap();
		assert_eq!(resp_str, expected_str);
	}

	#[test]
	fn test_serialize_error_response() {
		let resp = Response {
			jsonrpc: "2.0",
			id: Cow::Owned(to_raw_value(&0).unwrap()),
			result: None,
			error: Some(Cow::Owned(to_raw_value(&"null arg").unwrap())),
		};
		let expected_str = "{\"jsonrpc\":\"2.0\",\"id\":0,\"error\":\"null arg\"}";
		let resp_str = serde_json::to_string(&resp).unwrap();
		assert_eq!(resp_str, expected_str);
	}
}