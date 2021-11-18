use smart_leds_trait::{SmartLedsWrite, RGB8};
use std::io::{self, stdout};

use crossterm::{ExecutableCommand, style::{Print, SetForegroundColor, ResetColor, Color}};

use crate::error::Error;
use crate::config::LayoutConfig;

pub struct TerminalWrite {
	strip_lens: Vec<usize>,
}

impl TerminalWrite {
	pub fn new(layout: &LayoutConfig) -> Self {
		TerminalWrite {
			strip_lens: layout.pixel_locations.iter().map(|strip| strip.len()).collect(),
		}
	}

	fn write_with_io_error<T, I>(&mut self, mut iterator: T) -> Result<(), io::Error>
		where
			T: Iterator<Item=I>,
			I: Into<RGB8>,
	{
		for len in self.strip_lens.iter().cloned() {
			for color in iterator.by_ref().take(len) {
				let color = color.into();
				stdout()
					.execute(SetForegroundColor(Color::Rgb {r: color.r, g: color.g, b: color.b}))?
					.execute(Print("O"))?;
			}
			stdout()
				.execute(ResetColor)?
				.execute(Print("\n"))?;
		}
		Ok(())
	}
}

impl SmartLedsWrite for TerminalWrite {
	type Error = Error;
	type Color = RGB8;

	fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
		where
			T: Iterator<Item=I>,
			I: Into<Self::Color>,
	{
		self.write_with_io_error(iterator)
			.map_err(Error::TerminalOutput)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn layout_config() -> LayoutConfig {
		LayoutConfig {
			pixel_locations: vec![
				vec![(0.0, 0.0); 50],
				vec![(0.0, 0.0); 50],
			],
		}
	}

	#[test]
	fn test_write() {
		let layout = layout_config();
		let mut writer = TerminalWrite::new(&layout);
		writer.write(vec![RGB8::new(255, 0, 0); 100].into_iter()).unwrap();
	}
}