[![Build Status](https://travis-ci.org/fusion-engineering/spicat.svg?branch=master)](https://travis-ci.org/fusion-engineering/spicat)

# spicat

A command line tool to perform full-duplex SPI transactions.

Example:
```sh
$ echo -n 'Hello world!' | spicat /dev/spidev1.0 --speed 10000000
48 65 6c 6c 6f 20 74 68 65 72 65 21
```

This will read 'Hello world!' from standard input,
and send it over SPI to the connected device.
The response will be printed to standard output.

The output format depends on whether or not output is going to a terminal.
If it is, output is printed in hexadecimal format by default.
Otherwise, the raw bytes are printed by default.
This behaviour can be overridden with the `--format` option.

The transaction can be repeated a number of times with the `--repeat` option,
to stress-test an SPI bus or device.

The `--pre-delay` option can be used to add a delay after asserting the chip select,
before transmitting the data.
This can be useful to give an SPI device some time to react to the chip select.
Note that this wait time is implemented by the Linux kernel,
which may mean the exact delay can be a few microseconds longer than the requested value.

See `spicat --help` for a list of every available option.

## Install
Run `cargo install spicat` to install the tool with cargo.
