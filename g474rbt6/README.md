# STM32G474RBT6 template

This is a template for Rust firmware targeting the STM32G474RBT6. This chip is
used across many of the rover PCBs. The starter code includes a Cyphal node
that publishes the current state of the LED in their cycle and a cool ARGB LED
rainbow pattern.

If you need a more minimal starting point, look at `g474rbt6-minimal`.

## Development commands

Flash and run with defmt logging output:

```
cargo run --release
```

Flash, then run with both defmt logging output and a GDB server simultaneously:

```
cargo embed --release
```

(Note: if you have issues with the probe-rs GDB server, you may need to switch to OpenOCD.)

With GDB server running, attach GDB (sub in `gdb-multiarch` or `arm-none-eabi-gdb` as appropriate):

```
gdb -x attach.gdb
```

With GDB server running, attach GDB, reset and halt on first instruction:

```
gdb -x reset.gdb
```

## Adapting the template for your own crate

1. Change the crate name in `Cargo.toml`, `attach.gdb` and `reset.gdb`
2. Check notes below and in the source code and adjust configuration as needed
3. For complex crates, split things into modules

## Notes

- Requires [`flip-link`](https://github.com/knurling-rs/flip-link)
- Uses OWR's `embedded_common` (see `Cargo.toml`)
- `memory.x` assumes chips are in dual-bank mode (which they ship in)
- Release build uses slightly less than 18 KB of flash
- Uses Bluesat's version of `stm32g4xx-hal` and `fdcan`
    - Uses `main`, not `qspi`
- Uses `panic_probe`: program will HardFault on panic
    - Use `defmt::panic!` in your code instead of regular `panic!` to get
      panic logging to RTT (the probe-rs log view)
- Includes a Cyphal node with a whopping 64 KB of heap memory (adjust to taste)
    - Responds to standard Cyphal `COMMAND_RESTART`
    - Reports "LED hue" as an unsigned 8-bit integer at 20 Hz
    - Assumes you're using CAN FD, not classic CAN
- Change pins as per your board's schematic
