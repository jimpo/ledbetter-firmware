mod error;
mod ws2812b;

use clap::{Arg, App, SubCommand};
use gpio_cdev::{Chip, ChipIterator, chips, LineRequestFlags};

use crate::ws2812b::WS2812BWrite;
use crate::error::Error;

const GPIO_LABEL: &str = "gpa0";
const GPIO_LINE: u32 = 0;

fn main_result() -> Result<(), Error> {
    clap
    let mut chip = gpio_cdev::chips()?
        .flat_map(|chip_result| chip_result.ok())
        .find(|chip| chip.label() == GPIO_LABEL)
        .ok_or_else(|| Error::GpioCdevNotFound { label: GPIO_LABEL.into() })?;
    log::info!("Found GPIO cdev with label {} mapped to {}", GPIO_LABEL, chip.name());

    let line = chip.get_line(GPIO_LINE)?;
    let line_handle = line.request(LineRequestFlags::OUTPUT, 0, "ledbetter")?;
    line_handle.set_value(1)?;

    //let ws2812b = WS2812BWrite::new(line);
    //ChipIterator::
    // let mut chip = Chip::new("/dev/gpiochip0")?;
    // let line = chip.get_line(0)?;
    // WS2812BWrite::new(line);
    Ok(())
}

fn main() {
    env_logger::init();
    main_result()
        .unwrap_or_else(|err| panic!("{}", err))
}
