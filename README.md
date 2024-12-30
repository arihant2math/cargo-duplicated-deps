# Cargo Duplicated Dependencies

This is a simple tool to find duplicated dependencies in a Cargo project.
Duplicated dependencies are dependencies that are compiled with different versions.
This can happen when different dependencies require different versions of the same package.
This leads to larger binaries and slower compilation.
This tool parses the `Cargo.lock` file and finds duplicated dependencies and outputs their paths.

## Installation

```bash
cargo install cargo-duplicated-deps
```

## Usage

```bash
cargo duplicated-deps
```

