# STM32G431CBU6 training template

This is a template for Rust firmware targeting the STM32G431CBU6. This chip is
found on the [G431 WeAct dev boards](https://www.aliexpress.com/item/1005007079255256.html)
(from AliExpress/Alibaba) and is not actually used on the rover.

## Development commands

- Flash and run with defmt logging output: `cargo run --release`
- Flash, then run with defmt logging output and GDB simultaneously: `cargo embed --release`
    - Connect GDB with `gdb-multiarch -x cargo.gdb`

## Notes

- Requires probe-rs-tools and [`flip-link`](https://github.com/knurling-rs/flip-link)
- Release build uses slightly more than 2 KB of flash
- Uses `stm32g4xx-hal` from Git, not Bluesat's version; does not enable fdcan
- Uses `panic_probe`: program will HardFault on panic
    - Use `defmt::panic!` in your code instead of regular `panic!` to get panic
      logging to RTT (the probe-rs log view)
