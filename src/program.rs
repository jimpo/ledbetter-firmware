use palette::{encoding::Srgb, rgb::Rgb};
use smart_leds_trait::RGB8;

use crate::config::LayoutConfig;
use crate::error::Error;

pub type PixelVal = Rgb<Srgb, u8>;


pub trait Program {
	fn pixels(&self) -> &Vec<Vec<PixelVal>>;
	fn tick(&mut self) -> Result<(), Error>;

}

pub struct TrivialProgram {
	pixels: Vec<Vec<PixelVal>>,
}

impl TrivialProgram {
	pub fn new(layout: LayoutConfig) -> Self {
		let pixels = layout.pixel_locations.iter()
			.map(|strip_locations| vec![PixelVal::new(255, 255, 255); strip_locations.len()])
			.collect::<Vec<_>>();
		TrivialProgram {
			pixels,
		}
	}
}

impl Program for TrivialProgram {
	fn pixels(&self) -> &Vec<Vec<PixelVal>> {
		&self.pixels
	}

	fn tick(&mut self) -> Result<(), Error> {
		Ok(())
	}
}

pub fn leds_iter<'a>(program: &'a impl Program) -> impl Iterator<Item=RGB8> + 'a {
	program.pixels()[0].iter()
		.map(|rgb| RGB8::new(rgb.red, rgb.green, rgb.blue))
}