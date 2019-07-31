use spidev::{Spidev, SpiModeFlags, SpidevTransfer, SpidevOptions};
use std::io::{Read, Write};
use structopt::StructOpt;

enum OutputFormat {
	Decimal,
	Hexadecimal,
	Raw,
}

impl std::str::FromStr for OutputFormat {
	type Err = String;

	fn from_str(value: &str) -> Result<Self, String> {
		let lower = value.to_lowercase();
		match lower.as_str() {
			"raw"                 => Ok(OutputFormat::Raw),
			"hex" | "hexadecimal" => Ok(OutputFormat::Hexadecimal),
			"dec" | "decimal"     => Ok(OutputFormat::Decimal),
			_ => Err(format!("invalid output format, allowed values are: raw, hex or dec, got: {}", value)),
		}
	}
}

#[derive(StructOpt)]
#[structopt(author = "Fusion Engineering")]
#[structopt(raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
#[structopt(raw(setting = "structopt::clap::AppSettings::DeriveDisplayOrder"))]
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

	/// Print the response in the given format: raw, hex[adecimal] or dec[imal].
	#[structopt(long = "format", short = "f")]
	#[structopt(default_value = "hex")]
	format: OutputFormat,

	/// Bits per word for the SPI transaction.
	#[structopt(long = "bits")]
	#[structopt(value_name = "N")]
	#[structopt(default_value = "8")]
	bits_per_word: u8,

	/// Delay in microseconds after enabling the chip select line before sending data.
	#[structopt(long = "pre_delay")]
	#[structopt(value_name = "MICROSECONDS")]
	pre_delay: Option<u16>,
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
		// If we have a pre-delay, add a dummy write with delay_usecs and cs_change = 0.
		if let Some(pre_delay) = options.pre_delay {
			let mut transfers = [
				SpidevTransfer::write(&[]),
				SpidevTransfer::read_write(&tx_buf, &mut rx_buf),
			];

			transfers[0].cs_change   = 0;
			transfers[0].delay_usecs = pre_delay;
			transfers[0].speed_hz    = options.speed;
			transfers[1].speed_hz    = options.speed;
			spi.transfer_multiple(&mut transfers)?;

		// Else just do the single transfer.
		} else {
			let mut transfer  = SpidevTransfer::read_write(&tx_buf, &mut rx_buf);
			transfer.speed_hz = options.speed;
			spi.transfer(&mut transfer)?;
		}

		// Print the received data in the desired format.
		match options.format {
			OutputFormat::Raw => {
				let mut stdout = std::io::stdout();
				stdout.write_all(&rx_buf)?;
			},
			OutputFormat::Hexadecimal => {
				for (i, byte) in rx_buf.iter().enumerate() {
					if i != 0 {
						print!(" ");
					}
					print!("{:02X}", byte);
				}
				println!();
			},
			OutputFormat::Decimal => {
				for (i, byte) in rx_buf.iter().enumerate() {
					if i != 0 {
						print!(" ");
					}
					print!("{}", byte);
				}
				println!();
			},
		}
	}

	Ok(())
}
