use spidev::{Spidev, SpiModeFlags, SpidevTransfer, SpidevOptions};
use std::io::Read;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(author = "Fusion Engineering")]
struct Options {
	/// The spidev to open.
	#[structopt(value_name = "SPIDEV")]
	spidev: std::path::PathBuf,

	/// The speed in Hz for the SPI transaction.
	#[structopt(long = "speed", short = "s")]
	#[structopt(value_name = "HZ")]
	#[structopt(default_value = "1000000")]
	speed: u32,

	/// Repeat the transaction COUNT times,
	/// The speed in Hz for the SPI transaction.
	#[structopt(long = "repeat", short = "r")]
	#[structopt(value_name = "COUNT")]
	#[structopt(default_value = "1")]
	repeat: usize,

	/// Print the received response in hexadecimal.
	#[structopt(long = "hex", short = "x")]
	hex: bool,

	/// Bits per word for the SPI transaction.
	#[structopt(long = "bits")]
	#[structopt(value_name = "N")]
	#[structopt(default_value = "8")]
	bits_per_word: u8,
}

fn main() -> std::io::Result<()> {
	let options = Options::from_args();

	let spi_options = SpidevOptions::new()
		.bits_per_word(options.bits_per_word)
		.max_speed_hz(options.speed)
		.mode(SpiModeFlags::SPI_MODE_0)
		.build();

	let mut spi = Spidev::open(&options.spidev)?;
	spi.configure(&spi_options)?;

	let stdin = std::io::stdin();
	let mut stdin = stdin.lock();
	let mut tx_buf = Vec::new();
	stdin.read_to_end(&mut tx_buf)?;
	let mut rx_buf = Vec::new();
	rx_buf.resize(tx_buf.len(), 0u8);

	for _ in 0..options.repeat {
		let mut transfer = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
		transfer.speed_hz = options.speed;
		spi.transfer(&mut transfer)?;
		if options.hex {
			println!("{:02X?}", rx_buf);
		} else {
			println!("{:?}", rx_buf);
		}
	}

	Ok(())
}
