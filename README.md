# createXcrunch

createXcrunch is a Rust-based program designed to efficiently find zero-leading, zero-containing, or pattern-matching addresses for the [CreateX](https://github.com/pcaversaccio/createx) contract factory. Uses OpenCL in order to leverage a GPU's mining capabilities.

## Installation

1. **Clone the Repository**
```
git clone https://github.com/HrikB/createXcrunch
cd createXcrunch
```
2. **Build the Project**
```
cargo build --release
```

## Usage
```
./target/release/createxcrunch create3 --caller 0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A
  \ --crosschain 1
  \ --matching ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed
```

Use the `--help` flag for a full overview of all the features and how to use them.

## Contributions
PRs welcome!

## Acknowledgements
- https://github.com/0age/create2crunch
- https://github.com/Vectorized/function-selector-miner
