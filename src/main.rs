mod config;
mod control;
mod driver;
mod error;
mod jsonrpc;
mod program;
#[cfg(feature = "term_display")]
mod term_write;
mod wasm_program;
mod ws2812b_bitbang;
mod ws2812b_rpi;

use env_logger::Env;
use clap::{Arg, App};
use websocket::url::{ParseError, Url};

use std::{
	fs,
	process,
};

use crate::config::{Config, LayoutConfig};
use crate::control::{connect_and_process_with_reconnects, Controller};
use crate::driver::DriverImpl;
use crate::error::Error;
#[cfg(feature = "term_display")]
use crate::term_write::TerminalWrite;
#[cfg(not(feature = "term_display"))]
use crate::ws2812b_rpi::WS2812BRpiWrite;


fn get_controller_ws_url(config: &Config) -> Result<Url, Error> {
	let mut url = websocket::client::Url::parse("ws://example.com:443")
		.expect("static string is guaranteed to parse");
	url.set_host(Some(&config.controller.host))?;
	url.set_port(Some(config.controller.port))
		.map_err(|_| ParseError::InvalidPort)?;
	Ok(url)
}

fn main_result(config: Config) -> Result<(), Error> {
	#[cfg(feature = "term_display")]
	let ws2812b_factory = move |layout: &LayoutConfig| Ok(TerminalWrite::new(layout));
	#[cfg(not(feature = "term_display"))]
	let ws2812b_factory = {
		let gpio_lines = config.gpio_lines.clone();
		move |layout: &LayoutConfig| {
			WS2812BRpiWrite::new(gpio_lines.iter().cloned(), layout)
		}
	};
	// Try out constructor once here where we can fail fast
	let _ = ws2812b_factory(&config.layout)?;
	let driver = DriverImpl::new(ws2812b_factory, config.render_freq, config.layout.clone());
	let mut controller = Controller::new(&config.name, driver);

	let url = get_controller_ws_url(&config)?;
	connect_and_process_with_reconnects(&url, &mut controller);

	Ok(())
}

fn get_config() -> Config {
	let matches = App::new("LEDBetter Client")
		.version("1.0")
		.author("Jim Posen <jim.posen@gmail.com>")
		.about("LED driver client for LEDBetter lights")
		.arg(Arg::with_name("config")
			.short("c")
			.long("config")
			.value_name("FILE")
			.help("Path to the configuration file")
			.required(true)
			.takes_value(true))
		.get_matches();

	let config_path = matches.value_of("config").expect("config is required");
	let contents = fs::read_to_string(config_path)
		.unwrap_or_else(|err| {
			eprintln!("could not read config file {}: {}", config_path, err);
			process::exit(1);
		});

	let config: Config = toml::from_str(&contents)
		.unwrap_or_else(|err| {
			eprintln!("could not parse config file {}: {}", config_path, err);
			process::exit(1);
		});
	config
}

fn main() {
	env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
	let config = get_config();
	main_result(config)
		.unwrap_or_else(|err| panic!("{}", err))
}
