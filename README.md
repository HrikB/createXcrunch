# `createXcrunch`

[![👮‍♂️ Sanity checks](https://github.com/HrikB/createXcrunch/actions/workflows/checks.yml/badge.svg)](https://github.com/HrikB/createXcrunch/actions/workflows/checks.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/license/mit)

`createXcrunch` is a [Rust](https://www.rust-lang.org)-based program designed to efficiently find _zero-leading_, _zero-containing_, or _pattern-matching_ addresses for the [CreateX](https://github.com/pcaversaccio/createx) contract factory. Uses [OpenCL](https://www.khronos.org/opencl/) in order to leverage a GPU's mining capabilities.

## Installation

1. **Clone the Repository**

```console
git clone https://github.com/HrikB/createXcrunch.git
cd createXcrunch
```

2. **Build the Project**

```console
cargo build --release
```

> [!NOTE]
> Building on Windows works as long as you have installed the [CUDA Toolkit](https://docs.nvidia.com/cuda/cuda-installation-guide-microsoft-windows) or the [AMD Radeon Software](https://www.amd.com/en/resources/support-articles/faqs/RS-INSTALL.html). However, the [WSL 2](https://learn.microsoft.com/en-us/windows/wsl/install) installation on Windows `x64` systems with NVIDIA hardware fails, as the current NVIDIA driver does not yet support passing [OpenCL](https://en.wikipedia.org/wiki/OpenCL) to Windows Subsystem for Linux (WSL).

## Example Setup on [Vast.ai](https://vast.ai)

#### Update Linux

```console
sudo apt update && sudo apt upgrade
```

#### Install `build-essential` Packages

> We need the GNU Compiler Collection (GCC) later.

```console
sudo apt install build-essential
```

#### Install CUDA Toolkit

> `createXcrunch` uses [OpenCL](https://en.wikipedia.org/wiki/OpenCL) which is natively supported via the NVIDIA OpenCL extensions.

```console
sudo apt install nvidia-cuda-toolkit
```

#### Install Rust

> Enter `1` to select the default option and press the `Enter` key to continue the installation. Restart the current shell after completing the installation.

```console
curl https://sh.rustup.rs -sSf | sh
```

#### Build `createXcrunch`

```console
git clone https://github.com/HrikB/createXcrunch.git
cd createXcrunch
cargo build --release
```

🎉 Congrats, now you're ready to crunch your salt(s)!

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

## Local Development

We recommend using [`cargo-nextest`](https://nexte.st) as test runner for this repository. To install it on a Linux `x86_64` machine, invoke:

```console
curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ${CARGO_HOME:-~/.cargo}/bin
```

Afterwards you can run the tests via:

```console
cargo nextest run
```

## Contributions

PRs welcome!

## Acknowledgements

- [`create2crunch`](https://github.com/0age/create2crunch)
- [Function Selection Miner](https://github.com/Vectorized/function-selector-miner)
- [`CreateX` – A Trustless, Universal Contract Deployer](https://github.com/pcaversaccio/createx)
