[![Rust](https://github.com/yet-another-gauge/crankshaft/actions/workflows/rust.yml/badge.svg)](https://github.com/yet-another-gauge/crankshaft/actions/workflows/rust.yml)

# STM32F0 Crankshaft Monitor

Real-time crankshaft sensor monitoring system built with Rust and [embassy-rs](https://github.com/embassy-rs) framework for STM32F0 microcontrollers.

## Useful Commands

### Development

- Run with debug features using `probe-run`:
  ```bash
  cargo run --profile dev --bin crankshaft
  ```

### Analysis

- Analyze binary size with detailed section breakdown:
  ```bash
  cargo size --bin crankshaft --release --no-default-features -- -A
  ```

- Inspect read-only data section:
  ```bash
  cargo objdump --bin crankshaft --release --no-default-features -- -s -j .rodata | vi
  ```

> **Note**: Using `--no-default-features` disables debug functionality for smaller binary size and better performance.

### Release

- Flash release build:
  ```bash
  cargo embed --release --bin crankshaft --no-default-features
  ```

- Run release build:
  ```bash
  cargo run --profile release --bin crankshaft --no-default-features
  ```
