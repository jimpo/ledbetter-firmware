use websocket::{
	client::Url,
	OwnedMessage,
};
use serde::Deserialize;
use serde_json::Value;

use crate::error::Error;
use crate::jsonrpc;


#[derive(Debug, Clone, Deserialize)]
struct ReverseAuthParams<'a> {
	challenge: &'a str,
}

fn parse_params<'a, T: Deserialize<'a>>(request: &'a jsonrpc::Request) -> Result<T, Error> {
	serde_json::from_str(request.params.get()).map_err(Error::RequestDeserialization)
}

fn handle_reverse_auth(challenge: &str) -> Result<Value, Error> {
	Ok(Value::Null)
}

fn handle_request<'a>(request: &'a jsonrpc::Request) -> Result<jsonrpc::Response<'a>, Error> {
	request.validate().map_err(Error::BadJsonrpcRequest)?;
	let result =
		if request.method == "reverse_auth" {
			let params: ReverseAuthParams = parse_params(&request)?;
			handle_reverse_auth(params.challenge)?
		} else {
			return Err(Error::UnknownRpcMethod(request.method.to_string()));
		};
	let response = jsonrpc::Response {
		jsonrpc: "2.0",
		id: request.id,
		result: Some(result),
		error: None,
	};
	Ok(response)
}

pub fn connect(url: &Url) -> Result<(), Error> {
	let mut conn = websocket::ClientBuilder::from_url(&url)
		.connect_insecure()?;
	loop {
		let message = conn.recv_message()?;
		let request = match message {
			OwnedMessage::Text(ref msg) =>
				serde_json::from_str::<jsonrpc::Request>(msg)
					.map_err(Error::RequestDeserialization)?,
			_ => Err(Error::UnexpectedMessage(message))?,
		};
		let response = handle_request(&request)?;
		response.validate().map_err(Error::BadJsonrpcResponse);
		let response_ser = serde_json::to_string(&response)
			.map_err(Error::ResponseSerialization)?;
		conn.send_message(&OwnedMessage::Text(response_ser))?;
	}
}
