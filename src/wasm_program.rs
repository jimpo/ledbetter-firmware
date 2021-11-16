use palette::{FromColor, Hsv, encoding::Srgb, rgb::Rgb, rgb::channels::Argb, RgbHue};
use wasm3::{Environment, Function, Module, Runtime};

use crate::config::LayoutConfig;
use crate::error::Error;
use crate::program::{Program, PixelVal};

const STACK_SIZE: u32 = 1_000_000;

fn hsv_to_rgb_encoded(h: u32, s: u32, v: u32) -> u32 {
	let hsv = Hsv::new(RgbHue::from_degrees(h as f32), (s as f32) / 100.0, (v as f32) / 100.0);
	let rgb = <Rgb<Srgb, u8>>::from_format(Rgb::from_color(hsv));
	rgb.into_u32::<Argb>()
}

wasm3::make_func_wrapper!(
	hsv_to_rgb_encoded_wrapped: hsv_to_rgb_encoded(h: u32, s: u32, v: u32) -> u32
);


pub fn create_runtime() -> Result<Runtime, Error> {
	let wasm_env = Environment::new()?;
	let runtime = wasm_env.create_runtime(STACK_SIZE)?;
	Ok((runtime))
}

pub struct WasmProgram<'a> {
	pixels: Vec<Vec<PixelVal>>,
	module: Module<'a>,
	tick: Function<'a, (), ()>,
	get_pixel_red: Function<'a, (i32, i32), u32>,
	get_pixel_grn: Function<'a, (i32, i32), u32>,
	get_pixel_blu: Function<'a, (i32, i32), u32>,
}

fn make_pixels_array(layout: &LayoutConfig) -> Vec<Vec<PixelVal>> {
	layout.pixel_locations.iter()
		.map(|strip_locations| {
			vec![PixelVal::default(); strip_locations.len()]
		})
		.collect()
}

impl<'a> WasmProgram<'a> {
	pub fn new(layout: LayoutConfig, runtime: &'a Runtime, wasm_bin: Vec<u8>)
		-> Result<Self, Error>
	{
		let mut module = runtime.parse_and_load_module(wasm_bin)?;

		// This can be a closure since it doesn't need to be fast
		module.link_closure(
			"env", "abort",
			|ctx, (msg_ref, file_name_ref, line, column): (u32, u32, u32, u32)| {
				// TODO: Decode msg and fileName from instance memory
				log::warn!(
					"program aborted msgRef={}, fileNameRef={}, line={}, column={}",
					msg_ref, file_name_ref, line, column
				);
				Ok(())
			}
		)?;
		let link_result = module.link_function::<(u32, u32, u32), u32>(
			"colorConvert", "hsvToRgbEncoded",
			hsv_to_rgb_encoded_wrapped
		);
		match link_result {
			Ok(()) => {}
			Err(wasm3::error::Error::FunctionNotFound) => {}
			Err(err) => return Err(err.into()),
		}

		let init_layout_set_num_strips =
			module.find_function::<(i32), ()>("initLayoutSetNumStrips")?;
		let init_layout_set_strip_len =
			module.find_function::<(i32, i32), ()>("initLayoutSetStripLen")?;
		let init_layout_set_pixel_loc =
			module.find_function::<(i32, i32, f32, f32), ()>("initLayoutSetPixelLoc")?;
		let init_layout_done = module.find_function::<(), ()>("initLayoutDone")?;
		let tick = module.find_function::<(), ()>("tick")?;
		let get_pixel_red = module.find_function::<(i32, i32), u32>("getPixelRed")?;
		let get_pixel_grn = module.find_function::<(i32, i32), u32>("getPixelGrn")?;
		let get_pixel_blu = module.find_function::<(i32, i32), u32>("getPixelBlu")?;

		init_layout_set_num_strips.call(layout.pixel_locations.len() as i32)?;
		for (i, strip_locations) in layout.pixel_locations.iter().enumerate() {
			init_layout_set_strip_len.call(i as i32, strip_locations.len() as i32)?;
			for (j, (x, y)) in strip_locations.iter().enumerate() {
				init_layout_set_pixel_loc.call(i as i32, j as i32, *x, *y)?;
			}
		}
		init_layout_done.call()?;

		let mut program = WasmProgram {
			pixels: make_pixels_array(&layout),
			module,
			tick,
			get_pixel_red,
			get_pixel_grn,
			get_pixel_blu,
		};
		program.update_pixel_vals()?;
		Ok(program)
	}

	fn update_pixel_vals(&mut self) -> Result<(), Error> {
		for (i, strip_vals) in self.pixels.iter_mut().enumerate() {
			for (j, val) in strip_vals.iter_mut().enumerate() {
				let red = self.get_pixel_red.call(i as i32, j as i32)?;
				let grn = self.get_pixel_grn.call(i as i32, j as i32)?;
				let blu = self.get_pixel_blu.call(i as i32, j as i32)?;
				*val = PixelVal::new(red as u8, grn as u8, blu as u8);
			}
		}
		Ok(())
	}
}

impl<'a> Program for WasmProgram<'a> {
	fn pixels(&self) -> &Vec<Vec<PixelVal>> {
		&self.pixels
	}

	fn tick(&mut self) -> Result<(), Error> {
		self.tick.call()?;
		self.update_pixel_vals()?;
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	const TEST_PROGRAM: &[u8]  = include_bytes!("../testMain.wasm");

	fn layout_config() -> LayoutConfig {
		let ys = (0..150).map(|i| (i as f32) / 60.0).collect::<Vec<_>>();
		LayoutConfig {
			pixel_locations: vec![
				ys.iter().map(|&y| (-10.0, y)).collect(),
				ys.iter().map(|&y| (10.0, y)).collect(),
			],
		}
	}

	#[test]
	fn test_program_constructor() {
		let layout = layout_config();
		let runtime = create_runtime().unwrap();
		assert!(WasmProgram::new(layout, &runtime, TEST_PROGRAM.to_vec()).is_ok());
	}

	#[test]
	fn test_tick_and_render() {
		let layout = layout_config();
		let runtime = create_runtime().unwrap();
		let mut program = WasmProgram::new(layout, &runtime, TEST_PROGRAM.to_vec()).unwrap();
		assert_eq!(program.pixels(), &vec![vec![PixelVal::new(0, 0, 0); 150]; 2]);
		program.tick().unwrap();
		assert_eq!(program.pixels(), &vec![vec![PixelVal::new(255, 0, 0); 150]; 2]);
	}
}