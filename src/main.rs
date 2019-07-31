use spidev::{Spidev, SpiModeFlags, SpidevTransfer, SpidevOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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
	spidev: PathBuf,

	/// Read input from a file, or - for standard input.
	#[structopt(long = "in", short = "i")]
	#[structopt(value_name = "PATH")]
	#[structopt(default_value = "-")]
	input: PathBuf,

	/// Write output to a file, or - for standard output.
	#[structopt(long = "out", short = "o")]
	#[structopt(value_name = "PATH")]
	#[structopt(default_value = "-")]
	output: PathBuf,

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

	let stdin  = std::io::stdin();
	let stdout = std::io::stdout();

	let mut spi = Spidev::open(&options.spidev)?;
	spi.configure(&SpidevOptions::new()
		.bits_per_word(options.bits_per_word)
		.max_speed_hz(options.speed)
		.mode(SpiModeFlags::SPI_MODE_0)
		.build()
	)?;

	let mut input : Box<dyn Read> = if options.input == Path::new("-") {
		Box::new(stdin.lock())
	} else {
		Box::new(std::fs::File::open(&options.input)?)
	};

	let mut output : Box<dyn Write> = if options.output == Path::new("-") {
		Box::new(stdout.lock())
	} else {
		// Unlink file before opening, but ignore errors.
		// Then create a new file (fail if it still exists).
		let _ = std::fs::remove_file(&options.output);
		Box::new(std::fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&options.output)?
		)
	};

	let mut tx_buf = Vec::new();
	input.read_to_end(&mut tx_buf)?;
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
				output.write_all(&rx_buf)?;
			},
			OutputFormat::Hexadecimal => {
				for (i, byte) in rx_buf.iter().enumerate() {
					if i != 0 {
						write!(output, " ")?;
					}
					write!(output, "{:02X}", byte)?;
				}
				writeln!(output)?;
			},
			OutputFormat::Decimal => {
				for (i, byte) in rx_buf.iter().enumerate() {
					if i != 0 {
						write!(output, " ")?;
					}
					write!(output, "{}", byte)?;
				}
				writeln!(output)?;
			},
		}
	}

	Ok(())
}
