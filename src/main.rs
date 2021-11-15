mod config;
mod control;
mod driver;
mod error;
mod jsonrpc;
mod program;
mod wasm_program;
mod ws2812b;

use env_logger::Env;
use clap::{Arg, App};
use gpio_cdev::{LineRequestFlags};

use std::{
    fs,
    process,
    thread,
};

use crate::config::Config;
use crate::control::Controller;
use crate::driver::{Driver, DriverImpl};
use crate::error::Error;
use std::time::Duration;
use crate::ws2812b::WS2812BWrite;
use smart_leds_trait::{SmartLedsWrite, RGB8};


fn main_result(config: Config) -> Result<(), Error> {
    let mut chip = gpio_cdev::chips()?
        .flat_map(|chip_result| chip_result.ok())
        .find(|chip| chip.label() == config.gpio_label)
        .ok_or_else(|| Error::GpioCdevNotFound { label: config.gpio_label.clone() })?;
    log::info!("Found GPIO cdev with label {} mapped to {}", config.gpio_label, chip.name());

    let line = chip.get_line(config.gpio_line)?;
    let mut ws2812b = WS2812BWrite::new(line);
    ws2812b.write(vec![RGB8::new(255, 0, 0); 5].into_iter())?;
    //let mut driver = DriverImpl::new(ws2812b, config.render_freq, config.layout);
    //let controller = Controller::new(&config.name, driver);
    //driver.start(&[]);
    //thread::sleep(Duration::from_secs(60));

    // let mut url = websocket::client::Url::parse("ws://")
    //     .expect("static string ws:// is guaranteed to parse");
    // url.set_host(Some(&config.controller.host))?;
    // url.set_port(Some(config.controller.port))
    //     .unwrap();
    // control::connect(&url)?;

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
