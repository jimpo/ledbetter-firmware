use websocket::{WebSocketError, url::ParseError, OwnedMessage};

use crate::jsonrpc;

#[derive(Debug, derive_more::Display, derive_more::Error, derive_more::From)]
pub enum Error {
	#[from(ignore)]
	#[display(fmt = "No GPIO cdev found with label {}", label)]
	GpioCdevNotFound { label: String },
	GpioCdev(gpio_cdev::Error),
	UrlParseError(ParseError),
	WebSocketError(WebSocketError),
	RpiWS2111x(rs_ws281x::WS2811Error),
	// Can't hold wasm3 error type directly because it is !Send
	#[from(ignore)]
	Wasm3(#[error(not(source))] String),
	#[from(ignore)]
	#[display(fmt = "Unexpected message from controller: {:?}", _0)]
	UnexpectedMessage(#[error(not(source))] OwnedMessage),
	#[from(ignore)]
	#[display(fmt = "Unknown RPC method from controller: {:?}", _0)]
	UnknownRpcMethod(#[error(not(source))] String),
	#[from(ignore)]
	RequestSerialization(serde_json::Error),
	#[from(ignore)]
	RequestDeserialization(serde_json::Error),
	#[from(ignore)]
	ResponseDeserialization(serde_json::Error),
	#[from(ignore)]
	ResponseSerialization(serde_json::Error),
	#[from(ignore)]
	BadJsonrpcRequest(jsonrpc::Error),
	#[from(ignore)]
	BadJsonrpcResponse(jsonrpc::Error),
}

impl From<wasm3::error::Error> for Error {
	fn from(err: wasm3::error::Error) -> Self {
		Error::Wasm3(err.to_string())
	}
}