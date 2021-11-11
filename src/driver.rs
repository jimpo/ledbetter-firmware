use std::{
	thread,
	time::{Instant, Duration},
	sync::mpsc::{self, Receiver},
};
use gpio_cdev::Line;
use log;
use serde::{Deserialize, Serialize};

use crate::ws2812b::WS2812BWrite;


#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum Status {
	NotPlaying,
	Playing,
	Paused,
}

#[derive(Debug, Clone, Copy)]
pub enum CtrlAction {
	Play,
	Pause,
	Exit,
}

// #[derive(Debug, derive_more::Display, derive_more::Error)]
// pub enum Error {
// 	InvalidStatus(Status),
// }

#[cfg_attr(test, mockall::automock)]
pub trait Driver {
	fn start(&mut self) -> Status;
	fn stop(&mut self) -> Status;
	fn play(&mut self) -> Status;
	fn pause(&mut self) -> Status;
}

pub struct DriverImpl {
	gpio_line: Line,
	render_freq: usize,
	thread_handle: Option<thread::JoinHandle<()>>,
	ctrl_sender: Option<mpsc::SyncSender<CtrlAction>>,
	status: Status,
}

impl DriverImpl {
	pub fn new(gpio_line: Line, render_freq: usize) -> Self {
		DriverImpl {
			gpio_line,
			render_freq,
			thread_handle: None,
			ctrl_sender: None,
			status: Status::NotPlaying,
		}
	}
}

impl Driver for DriverImpl {
	fn start(&mut self) -> Status {
		if let None = self.thread_handle {
			let (sender, receiver) = mpsc::sync_channel(1);
			let line = self.gpio_line.clone();
			let render_period = Duration::from_millis((1000 / self.render_freq) as u64);
			self.thread_handle = thread::spawn(move || {
				run_driver(line, render_period, receiver)
			}).into();
			self.ctrl_sender = Some(sender);
			self.status = Status::Playing;
		}
		self.status
	}

	fn stop(&mut self) -> Status {
		match (self.thread_handle.take(), self.ctrl_sender.take()) {
			(Some(thread_handle), Some(ctrl_sender)) => {
				if let Err(err) = ctrl_sender.send(CtrlAction::Exit) {
					log::error!("could not send Exit message to driver thread: {}", err);
				}
				if let Err(_) = thread_handle.join() {
					log::error!("driver thread panicked");
				}
				self.status = Status::NotPlaying;
			},
			_ => {}
		}
		self.status
	}

	fn play(&mut self) -> Status {
		if let Some(ref mut ctrl_sender) = self.ctrl_sender {
			if let Err(err) = ctrl_sender.send(CtrlAction::Play) {
				log::error!("could not send Play message to driver thread: {}", err);
			}
			self.status = Status::Playing;
		}
		self.status
	}

	fn pause(&mut self) -> Status {
		if let Some(ref mut ctrl_sender) = self.ctrl_sender {
			if let Err(err) = ctrl_sender.send(CtrlAction::Pause) {
				log::error!("could not send Pause message to driver thread: {}", err);
			}
			self.status = Status::Paused;
		}
		self.status
	}
}

fn run_driver(line: Line, render_period: Duration, ctrl_receiver: Receiver<CtrlAction>) {
	let ws2812b = WS2812BWrite::new(line);
	let mut playing = true;
	let mut render_at = Instant::now();

	loop {
		let timeout = render_at.saturating_duration_since(Instant::now());
		match ctrl_receiver.recv_timeout(timeout) {
			Ok(CtrlAction::Play) => playing = true,
			Ok(CtrlAction::Pause) => playing = false,
			Ok(CtrlAction::Exit) => break,
			Err(mpsc::RecvTimeoutError::Disconnected) => {
				log::warn!("Driver control channel unexpectedly disconnected");
				return;
			},
			Err(mpsc::RecvTimeoutError::Timeout) => {
				if playing {
					// Do stuff
				}
				render_at += render_period;
			},
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::value::Value;

	#[test]
	fn test_status_serialization() {
		assert_eq!(
			serde_json::to_value(&Status::NotPlaying).unwrap(),
			Value::String("NotPlaying".into())
		);
		assert_eq!(
			serde_json::to_value(&Status::Playing).unwrap(),
			Value::String("Playing".into())
		);
		assert_eq!(
			serde_json::to_value(&Status::Paused).unwrap(),
			Value::String("Paused".into())
		);
	}
}