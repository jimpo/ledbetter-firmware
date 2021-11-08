mod control;
mod error;
mod jsonrpc;
mod ws2812b;

use clap::{Arg, App, SubCommand};
use gpio_cdev::{Chip, ChipIterator, chips, LineRequestFlags};
use serde::{Deserialize};

use std::{
    fs::File,
    process,
};
use std::io::Read;
use websocket::url::Url;

use crate::ws2812b::WS2812BWrite;
use crate::error::Error;
use crate::control::connect;


#[derive(Debug, Clone, Deserialize)]
struct ControllerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    gpio_label: String,
    gpio_line: u32,
    controller: ControllerConfig,
}

fn main_result(config: Config) -> Result<(), Error> {
    let mut chip = gpio_cdev::chips()?
        .flat_map(|chip_result| chip_result.ok())
        .find(|chip| chip.label() == config.gpio_label)
        .ok_or_else(|| Error::GpioCdevNotFound { label: config.gpio_label.clone() })?;
    log::info!("Found GPIO cdev with label {} mapped to {}", config.gpio_label, chip.name());

    let line = chip.get_line(config.gpio_line)?;
    let line_handle = line.request(LineRequestFlags::OUTPUT, 0, "ledbetter")?;
    line_handle.set_value(1)?;

    let mut url = websocket::client::Url::parse("ws://")
        .expect("static string ws:// is guaranteed to parse");
    url.set_host(Some(&config.controller.host))?;
    url.set_port(Some(config.controller.port))
        .unwrap();
    connect(&url)?;
    //let ws2812b = WS2812BWrite::new(line);
    //ChipIterator::
    // let mut chip = Chip::new("/dev/gpiochip0")?;
    // let line = chip.get_line(0)?;
    // WS2812BWrite::new(line);
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
    let mut file = File::open(config_path)
        .unwrap_or_else(|err| {
            eprintln!("could not open config file {}: {}", config_path, err);
            process::exit(1);
        });

    let mut contents = String::new();
    let _ = file.read_to_string(&mut contents)
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
    env_logger::init();
    let config = get_config();
    main_result(config)
        .unwrap_or_else(|err| panic!("{}", err))
}
