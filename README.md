# chip_8_emu

A CHIP-8 interpreter written in Rust with [macroquad](https://macroquad.rs/) for rendering and audio. Targets S-CHIP variant (CHIP-48) semantics.

## ROMs

Only the PONG game is included, but you can download more games and programs [here](https://github.com/kripod/chip8-roms).

## Requirements

- A `beep.wav` file at `assets/` (optional, the emulator runs silently if missing). If the project is compiled, the sound is embedded in the binary.

## Usage

Running it

```sh
cargo run -- <path-to-rom>
```

Building it

```sh
cargo build --release
mv target/build/release/chip_8_emu . # The binary is at target/build/release
./chip_8_emu <path-to-rom>
```

## Key Mapping

| CHIP-8 | Keyboard |
| --- | --- |
| `1` | `1` |		
| `2` | `2` |		
| `3` | `3` |		
| `C` | `4` |		
| `7` | `A` |		
| `8` | `S` |		
| `9` | `D` |		
| `E` | `F` |		
| `4` | `Q` |
| `5` | `W` |
| `6` | `E` |
| `D` | `R` |
| `A` | `Z` |
| `0` | `X` |
| `B` | `C` |
| `F` | `V` |

## Testing

```sh
cargo test
```

Covers every opcode's decode and execution semantics, including VF edge cases, memory wrapping, timer catch-up, and the SetToKey state machine.

## Limitations

- No SCHIP extensions (64×128 hi-res mode, FX75/FX85 flag registers, scrolling)
- Mono beep only; no pitch or waveform variety

## License

GPL 3.0

## References

- [CHIP-8's Wikipedia page](https://en.wikipedia.org/wiki/CHIP-8)
- [CHIP-8 Research Facility](https://chip-8.github.io/)
- [CHIP-8 Variant Opcode Table](https://chip8.gulrak.net/)
- [A collection of CHIP-8 programs and documentation.](https://github.com/mattmikolay/chip-8)
- [ROM for testing chip8 emulator](https://github.com/corax89/chip8-test-rom/tree/master)