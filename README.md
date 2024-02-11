# `createXcrunch`

[![👮‍♂️ Sanity checks](https://github.com/HrikB/createXcrunch/actions/workflows/checks.yml/badge.svg)](https://github.com/HrikB/createXcrunch/actions/workflows/checks.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/license/mit/)

`createXcrunch` is a [Rust](https://www.rust-lang.org)-based program designed to efficiently find _zero-leading_, _zero-containing_, or _pattern-matching_ addresses for the [CreateX](https://github.com/pcaversaccio/createx) contract factory. Uses [OpenCL](https://www.khronos.org/opencl/) in order to leverage a GPU's mining capabilities.

## Installation

1. **Clone the Repository**

```console
git clone https://github.com/HrikB/createXcrunch
cd createXcrunch
```

2. **Build the Project**

```console
cargo build --release
```

> Building on Windows currently fails (see [this](https://github.com/HrikB/createXcrunch/issues/1) issue). If you want to continue using Windows, we recommend using the Windows Subsystem for Linux (WSL) and installing Rust via `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`.

## Usage

```console
./target/release/createxcrunch create3 --caller 0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A
  \ --crosschain 1
  \ --matching ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed
```

Use the `--help` flag for a full overview of all the features and how to use them:

```console
./target/release/createxcrunch create2 --help
```

or

```console
./target/release/createxcrunch create3 --help
```

## Contributions

PRs welcome!

## Acknowledgements

- [`create2crunch`](https://github.com/0age/create2crunch)
- [Function Selection Miner](https://github.com/Vectorized/function-selector-miner)
- [`CreateX` – A Trustless, Universal Contract Deployer](https://github.com/pcaversaccio/createx)
