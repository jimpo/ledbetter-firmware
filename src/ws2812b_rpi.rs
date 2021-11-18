use rs_ws281x::{ChannelBuilder, Controller, ControllerBuilder, StripType};
use smart_leds_trait::{SmartLedsWrite, RGB8};
use std::time::{Duration, Instant};

use crate::config::LayoutConfig;
use crate::error::Error;

#[derive(Debug, Clone)]
pub struct WS2812BRpiWrite {
	controller: Controller,
}

const MAX_WAIT_TIME: Duration = Duration::from_millis(1);

impl WS2812BRpiWrite {
	pub fn new<I: Iterator<Item=u32>>(pins: I, layout: &LayoutConfig) -> Result<Self, Error> {
		let mut controller_builder = ControllerBuilder::new();
		controller_builder.freq(1_000_000_000 / 1250);  // 1250ns period

		let strip_lens = layout.pixel_locations.iter().map(|locs| locs.len());
		for (i, (pin_no, strip_len)) in pins.zip(strip_lens).enumerate() {
			let channel = ChannelBuilder::new()
				.pin(pin_no as i32)
				.count(strip_len as i32)
				.strip_type(StripType::Ws2812)
				.brightness(255)
				.build();
			controller_builder.channel(i, channel);
		}

		let controller = controller_builder.build()?;
		Ok(WS2812BRpiWrite {
			controller,
		})
	}
}

impl SmartLedsWrite for WS2812BRpiWrite {
	type Error = Error;
	type Color = RGB8;

	fn write<T, I>(&mut self, mut iter: T) -> Result<(), Self::Error>
		where
			T: Iterator<Item=I>,
			I: Into<Self::Color>,
	{
		let before_wait = Instant::now();
		self.controller.wait()?;
		if Instant::now().duration_since(before_wait) > MAX_WAIT_TIME {
			log::warn!(
				"Had to wait more than {}us for last render before next render, \
				render frequency may be too high",
				MAX_WAIT_TIME.as_millis()
			);
		}

		for channel in self.controller.channels() {
			for value in self.controller.leds_mut(channel).iter_mut() {
				if let Some(color) = iter.next() {
					let color = color.into();
					*value = [color.b, color.g, color.r, 0];
				} else {
					log::error!("Not enough values in color iterator");
					break;
				}
			}
		}
		self.controller.render()?;
		Ok(())
	}
}