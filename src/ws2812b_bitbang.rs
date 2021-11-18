use smart_leds_trait::{RGB8, SmartLedsWrite};
use gpio_cdev::{Line, LineRequestFlags, LineHandle};
use std::{
	thread,
	time::Duration,
};

use crate::error::Error;

const T0H_NS: u32 = 400;
const T0L_NS: u32 = 850;
const T1H_NS: u32 = 850;
const T1L_NS: u32 = 400;
const RESET_NS: u32 = 50 * 1000;


fn find_line_from_label_and_offset(label: &str, line_offset: u32) -> Result<Line, Error> {
	let mut chip = gpio_cdev::chips()?
		.flat_map(|chip_result| chip_result.ok())
		.find(|chip| chip.label() == label)
		.ok_or_else(|| Error::GpioCdevNotFound { label: label.to_string() })?;
	log::info!("Found GPIO cdev with label {} mapped to {}", label, chip.name());

	let line = chip.get_line(line_offset)?;
	Ok(line)
}

#[derive(Debug, Clone)]
pub struct WS2812BGpioBitbangWrite {
	line: Line,
	clk_freq: u32,
}

// This is too slow to actually work
impl WS2812BGpioBitbangWrite {
	pub fn new(line: Line) -> Self {
		WS2812BGpioBitbangWrite {
			line,
			clk_freq: 1_000_000_000,
		}
	}

	pub fn from_label_and_line_offset(label: &str, line_offset: u32) -> Result<Self, Error> {
		let line = find_line_from_label_and_offset(label, line_offset)?;
		Ok(Self::new(line))
	}
}

struct WS2812BWriteSession {
	line_handle: LineHandle,
	t0h_cycles: u32,
	t0l_cycles: u32,
	t1h_cycles: u32,
	t1l_cycles: u32,
	reset_cycles: u32,
}

fn delay_cycles(cycles: u32) {
	thread::sleep(Duration::from_nanos(cycles as u64));
}

fn ns_to_cycles(ns: u32, clk_freq: u32) -> u32 {
	((clk_freq as u64) * (ns as u64) / 1_000_000_000u64) as u32
}

impl WS2812BWriteSession {
	fn write_bit(&self, b: bool) -> Result<(), Error> {
		self.line_handle.set_value(1)?;
		delay_cycles(if b { self.t1h_cycles } else { self.t0h_cycles });
		self.line_handle.set_value(0)?;
		delay_cycles(if b { self.t1l_cycles } else { self.t0l_cycles });
		Ok(())
	}

	fn write_byte(&self, b: u8) -> Result<(), Error> {
		for i in (0..8).rev() {
			self.write_bit((b >> i) & 1 != 0)?;
		}
		Ok(())
	}

	fn write_pixel(&self, pixel: RGB8) -> Result<(), Error> {
		self.write_byte(pixel.g)?;
		self.write_byte(pixel.r)?;
		self.write_byte(pixel.b)?;
		Ok(())
	}

	fn flush(&self) -> Result<(), Error> {
		delay_cycles(self.reset_cycles);
		Ok(())
	}
}

impl SmartLedsWrite for WS2812BGpioBitbangWrite {
	type Error = Error;
	type Color = RGB8;

	fn write<T, I>(&mut self, iterator: T) -> Result<(), Self::Error>
		where
			T: Iterator<Item=I>,
			I: Into<Self::Color>,
	{
		let line_handle = self.line.request(LineRequestFlags::OUTPUT, 0, "ledbetter")?;
		let session = WS2812BWriteSession {
			line_handle,
			t0h_cycles: ns_to_cycles(T0H_NS, self.clk_freq),
			t0l_cycles: ns_to_cycles(T0L_NS, self.clk_freq),
			t1h_cycles: ns_to_cycles(T1H_NS, self.clk_freq),
			t1l_cycles: ns_to_cycles(T1L_NS, self.clk_freq),
			reset_cycles: ns_to_cycles(RESET_NS, self.clk_freq),
		};
		for pixel in iterator {
			session.write_pixel(pixel.into())?;
		}
		session.flush()?;
		Ok(())
	}
}