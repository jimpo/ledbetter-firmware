use websocket::{
	client::Url,
	OwnedMessage,
};
use serde::{Deserialize, Serialize};
use serde_json::value::{Value, RawValue};
use std::borrow::Cow;
use websocket::{
	stream::{
		sync::{AsTcpStream, TcpStream},
		Stream,
	},
	sync::Client,
};

use crate::error::Error;
use crate::jsonrpc;

pub enum Request {
	ReverseAuth(ReverseAuthParams),
	Play,
	Pause,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReverseAuthParams {
	pub challenge: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReverseAuthResult {
	pub name: String,
}

impl Request {
	pub fn from_jsonrpc(jsonrpc_req: &jsonrpc::Request) -> Result<Self, Error> {
		jsonrpc_req.validate().map_err(Error::BadJsonrpcRequest)?;
		if jsonrpc_req.method == "reverse_auth" {
			Ok(Request::ReverseAuth(parse_params(&jsonrpc_req)?))
		} else if jsonrpc_req.method == "play" {
			let _ = parse_params::<()>(&jsonrpc_req)?;
			Ok(Request::Play)
		} else if jsonrpc_req.method == "pause" {
			let _ = parse_params::<()>(&jsonrpc_req)?;
			Ok(Request::Pause)
		} else {
			Err(Error::UnknownRpcMethod(jsonrpc_req.method.to_string()))
		}
	}

	pub fn to_jsonrpc(&self, id: u32) -> Result<jsonrpc::Request, Error> {
		let (method, params_ser_result) = match self {
			Request::ReverseAuth(params) =>
				("reverse_auth", serde_json::to_string(params)),
			Request::Play =>
				("play", serde_json::to_string(&())),
			Request::Pause =>
				("pause", serde_json::to_string(&())),
		};
		let params_ser = params_ser_result.map_err(Error::RequestSerialization)?;
		let params = RawValue::from_string(params_ser)
			.map_err(Error::RequestSerialization)?;
		let request = jsonrpc::Request {
			jsonrpc: "2.0",
			id,
			method,
			params: Cow::Owned(params),
		};
		Ok(request)
	}
}

pub struct Controller {
	driver_name: String,
}

impl Controller {
	pub fn new<S: ToString>(driver_name: S) -> Self {
		Controller {
			driver_name: driver_name.to_string(),
		}
	}

	pub fn driver_name(&self) -> &str {
		&self.driver_name
	}

	pub fn handle_reverse_auth(&self, _params: &ReverseAuthParams) -> ReverseAuthResult {
		ReverseAuthResult {
			name: self.driver_name.clone(),
		}
	}

	pub fn handle_play(&self) {

	}

	pub fn handle_pause(&self) {

	}
}

fn parse_params<'a, T: Deserialize<'a>>(request: &'a jsonrpc::Request) -> Result<T, Error> {
	serde_json::from_str(request.params.get()).map_err(Error::RequestDeserialization)
}

fn handle_request<'a>(controller: &'a Controller, request: &'a jsonrpc::Request)
	-> Result<jsonrpc::Response<'a>, Error>
{
	let result = match Request::from_jsonrpc(request)? {
		Request::ReverseAuth(params) => {
			let result = controller.handle_reverse_auth(&params);
			serde_json::to_value(result)
		},
		Request::Play => {
			controller.handle_play();
			serde_json::to_value("OK")
		},
		Request::Pause => {
			controller.handle_pause();
			serde_json::to_value("OK")
		},
	};
	let response = jsonrpc::Response {
		jsonrpc: "2.0",
		id: request.id,
		result: Some(result.map_err(Error::ResponseSerialization)?),
		error: None,
	};
	Ok(response)
}

pub fn connect(url: &Url) -> Result<Connection<TcpStream>, Error> {
	let client = websocket::ClientBuilder::from_url(&url)
		.connect_insecure()?;
	Ok(Connection { client })
}

pub struct Connection<S>
	where S: AsTcpStream + Stream
{
	client: Client<S>,
}

impl<S> Connection<S>
	where S: AsTcpStream + Stream
{
	pub fn process_one(&mut self, controller: &Controller) -> Result<(), Error> {
		let message = self.client.recv_message()?;
		let request = match message {
			OwnedMessage::Text(ref msg) =>
				serde_json::from_str::<jsonrpc::Request>(msg)
					.map_err(Error::RequestDeserialization)?,
			_ => Err(Error::UnexpectedMessage(message))?,
		};
		let response = handle_request(controller, &request)?;
		response.validate().map_err(Error::BadJsonrpcResponse);
		let response_ser = serde_json::to_string(&response)
			.map_err(Error::ResponseSerialization)?;
		self.client.send_message(&OwnedMessage::Text(response_ser))?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use websocket::server::{WsServer, NoTlsAcceptor};
	use std::net::TcpListener;
	use std::thread;

	struct ServerConnection {
		client: Client<TcpStream>,
		request_id: u32,
	}

	impl ServerConnection {
		fn accept(server: &mut WsServer<NoTlsAcceptor, TcpListener>) -> Self {
			let client = server.accept().unwrap().accept().unwrap();
			ServerConnection {
				client,
				request_id: 0,
			}
		}

		fn send_request(&mut self, request: Request) -> Result<Result<Value, Value>, Error> {
			let request_id = self.request_id;
			self.request_id += 1;

			let jsonrpc_req = request.to_jsonrpc(request_id)?;
			let request_ser = serde_json::to_string(&jsonrpc_req)
				.map_err(Error::RequestSerialization)?;
			self.client.send_message(&OwnedMessage::Text(request_ser))?;
			let message = self.client.recv_message()?;
			let jsonrpc_resp = match message {
				OwnedMessage::Text(ref msg) => serde_json::from_str::<jsonrpc::Response>(msg)
					.map_err(Error::ResponseDeserialization)?,
				_ => Err(Error::UnexpectedMessage(message))?,
			};
			jsonrpc_resp.validate().map_err(Error::BadJsonrpcResponse)?;
			assert_eq!(jsonrpc_resp.id, jsonrpc_req.id);
			if let Some(result) = jsonrpc_resp.result {
				return Ok(Ok(result));
			}
			if let Some(error) = jsonrpc_resp.error {
				return Ok(Err(error));
			}
			Err(Error::BadJsonrpcResponse(jsonrpc::Error::ResponseHasNeitherResultNorError))
		}
	}

	#[test]
	fn test_connect_process_one() {
		let controller = Controller::new("test");
		let mut server = <WsServer<NoTlsAcceptor, TcpListener>>::bind("127.0.0.1:7357").unwrap();
		let server_join_handle = thread::spawn(move || {
			let mut server_conn = ServerConnection::accept(&mut server);
			let request = Request::ReverseAuth(ReverseAuthParams {
				challenge: "476b76368dbd5028c2f371d2a7018e32".to_string(),
			});
			let result = server_conn.send_request(request).unwrap();
			let expected = ReverseAuthResult { name: "test".to_string() };
			assert_eq!(result, Ok(serde_json::to_value(&expected).unwrap()));
		});

		let mut conn = connect(&Url::parse("ws://127.0.0.1:7357").unwrap()).unwrap();
		conn.process_one(&controller).unwrap();
		server_join_handle.join().unwrap();
	}

	#[test]
	fn test_parse_null_params() {
		serde_json::from_str::<()>("null").unwrap()
	}
}