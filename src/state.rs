use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    time::{Duration, Instant},
};

use macroquad::{
    audio::{PlaySoundParams, Sound, play_sound, stop_sound},
    input::KeyCode,
};
use rand::{RngExt, rng, rngs::ThreadRng};

use crate::operation::Operation;

pub struct State {
    /// 4 KB memory (4096 bytes, addresses 0x000..=0xFFF)
    memory: [u8; 0x1000],
    /// 16 8-bit registers
    registers: [u8; 16],
    /// 16-bit register for 12-bit addresses
    memory_address: u16,
    /// Instruction pointer
    instruction_pointer: u16,
    /// Stack for 12-bit addresses
    stack: Vec<u16>,
    /// Pixel data (on or off) for the 64x32 screen
    pub screen_data: [[u8; 32]; 64],
    /// Delay timer
    delay_timer: u8,
    /// Sound timer
    sound_timer: u8,
    /// Timer of the last timer decrement, used to make the timers work in 60 Hz
    last_timer_tick: std::time::Instant,
    /// Sound to play when it's supposed to be beeping
    beeping_sound: Option<Sound>,
    /// Flag to know if the game is beeping
    is_beeping: bool,
    /// Key mappings from keyboard code to CHIP 8 key
    pub key_mappings: HashMap<KeyCode, usize>,
    /// Flag to know if the CPU is waiting for a key, used to avoid thread blocking. The `usize` is the
    /// register number where the key value must be stored
    pub waiting_for_key: Option<usize>,
    /// 16 values where `keys[key]` represents if `key` is pressed
    pub keys: [bool; 16],
    /// Random generator
    rng: ThreadRng,
}

impl State {
    /// Initial address is set at 0x200 since in the original chip 8 computers, the emulator took the first
    /// 0x200 bytes. For modern implementations, there is no need to do this and the counter could start at 0x000
    const INITIAL_MEMORY_ADDRESS: u16 = 0x200;

    /// Initial address for the font glyphs
    const FONT_BASE: u16 = 0x050;

    /// Timers are updated at 60 Hz
    const TIMER_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

    /// 4x5 font glyphs from 0-F
    const FONT: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80, // F
    ];

    /// Create a new [`State`]
    pub fn new(beeping_sound: Sound) -> Self {
        Self {
            beeping_sound: Some(beeping_sound),
            ..Default::default()
        }
    }

    /// Load a game into memory
    ///
    /// # Arguments
    ///
    /// - `path` (`&str`) - Path of the binary
    ///
    /// # Returns
    ///
    /// - `std::io::Result<usize>` - The number of bytes read
    ///
    /// # Errors
    ///
    /// This function will return an error if `path` does't exist or if the file is larger than the `State` memory
    pub fn load_game(&mut self, path: &str) -> std::io::Result<usize> {
        let bytes = fs::read(path)?;

        if bytes.len() > self.memory.len() - Self::INITIAL_MEMORY_ADDRESS as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ROM too large",
            ));
        }

        self.memory[Self::INITIAL_MEMORY_ADDRESS as usize..][..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    /// When the `sound_timer > 0`, emit a beep sound
    pub fn beep(&mut self) {
        if self.sound_timer > 0 && !self.is_beeping {
            if let Some(beeping_sound) = &self.beeping_sound {
                play_sound(
                    beeping_sound,
                    PlaySoundParams {
                        looped: true,
                        volume: 1.0,
                    },
                );
            }
            self.is_beeping = true;
        } else if self.sound_timer == 0 && self.is_beeping {
            if let Some(beeping_sound) = &self.beeping_sound {
                stop_sound(beeping_sound);
            }
            self.is_beeping = false;
        }
    }

    /// Read `self.memory` at `self.counter` and  `self.counter + 1` (since opcodes are 16 bits) and get the next
    /// operation to execute
    ///
    /// # Returns
    ///
    /// - `Result<Operation, String>` - Operation to execute
    ///
    /// # Errors
    ///
    /// This function return an `Error` if the Opcode is not valid
    pub fn get_next_operation(&mut self) -> Result<Operation, String> {
        if let Some(register) = self.waiting_for_key {
            Ok(Operation::SetToKey(register))
        } else {
            let counter = self.instruction_pointer as usize;
            if counter + 1 >= self.memory.len() {
                return Err(format!("PC out of bounds: 0x{counter:03X}"));
            }

            let opcode = ((self.memory[counter] as u16) << 8u16) | self.memory[counter + 1] as u16;
            self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;

            Operation::try_from(opcode)
        }
    }

    /// Update the [`State`] according to the [`Operation`] that the program requires
    ///
    /// # Arguments
    ///
    /// - `operation` (`Operation`) - Operation to execute
    pub fn perform_operation(&mut self, operation: Operation) {
        match operation {
            Operation::CallAssemblySubroutine(_address) => {
                // No-op
            }

            Operation::ClearDisplay => {
                self.screen_data = [[0; 32]; 64];
            }

            Operation::Return => {
                if let Some(address) = self.stack.pop() {
                    self.instruction_pointer = address;
                }
            }

            Operation::Goto(address) => self.instruction_pointer = address,

            Operation::CallSubroutine(address) => {
                self.stack.push(self.instruction_pointer);
                self.instruction_pointer = address;
            }

            Operation::SkipIfEquals(register, value) => {
                if self.registers[register] == value {
                    self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;
                }
            }

            Operation::SkipIfNotEquals(register, value) => {
                if self.registers[register] != value {
                    self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;
                }
            }

            Operation::SkipIfRegistersEquals(reg_x, reg_y) => {
                if self.registers[reg_x] == self.registers[reg_y] {
                    self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;
                }
            }

            Operation::AssignValue(register, value) => self.registers[register] = value,

            Operation::AddValue(register, value) => {
                self.registers[register] = self.registers[register].wrapping_add(value);
            }

            Operation::AssignRegister(reg_x, reg_y) => {
                self.registers[reg_x] = self.registers[reg_y]
            }

            Operation::BitOrAssignRegister(reg_x, reg_y) => {
                self.registers[reg_x] |= self.registers[reg_y]
            }

            Operation::BitAndAssignRegister(reg_x, reg_y) => {
                self.registers[reg_x] &= self.registers[reg_y]
            }

            Operation::BitXorAssignRegister(reg_x, reg_y) => {
                self.registers[reg_x] ^= self.registers[reg_y]
            }

            Operation::AddAssignRegister(reg_x, reg_y) => {
                let (sum, overflow) = self.registers[reg_x].overflowing_add(self.registers[reg_y]);
                self.registers[reg_x] = sum;
                self.registers[0xF] = if overflow { 1 } else { 0 };
            }

            Operation::SubAssignRegister(reg_x, reg_y) => {
                let (result, borrow) = self.registers[reg_x].overflowing_sub(self.registers[reg_y]);
                self.registers[reg_x] = result;
                self.registers[0xF] = if borrow { 0 } else { 1 };
            }

            Operation::ShiftRAssignRegister(reg_x, reg_y) => {
                // In some version (like this) this is is VX = VY >> 1. Others use VX = VX >> 1
                let src = self.registers[reg_y];
                self.registers[reg_x] = src >> 1;
                self.registers[0xF] = src & 0x1;
            }

            Operation::InvSubAssignRegister(reg_x, reg_y) => {
                let (result, borrow) = self.registers[reg_y].overflowing_sub(self.registers[reg_x]);
                self.registers[reg_x] = result;
                self.registers[0xF] = if borrow { 0 } else { 1 };
            }

            Operation::ShiftLAssignRegister(reg_x, reg_y) => {
                // In some version (like this) this is is VX = VY << 1. Others use VX = VX << 1
                let src = self.registers[reg_y];
                self.registers[reg_x] = src.wrapping_shl(1);
                self.registers[0xF] = (src >> 7) & 0x1;
            }

            Operation::SkipIfRegistersNotEquals(reg_x, reg_y) => {
                if self.registers[reg_x] != self.registers[reg_y] {
                    self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;
                }
            }

            Operation::SetI(value) => self.memory_address = value & 0xFFF,

            Operation::GotoPlusVX(register, address) => {
                self.instruction_pointer = (address + self.registers[register] as u16) & 0x0FFF;
            }

            Operation::Random(register, value) => {
                self.registers[register] = self.rng.random::<u8>() & value
            }

            Operation::Draw(reg_x, reg_y, height) => {
                let coord_x = self.registers[reg_x];
                let coord_y = self.registers[reg_y];

                self.registers[0xf] = 0;

                // loop for the amount of vertical lines needed to draw
                for y_line in 0..height {
                    let data =
                        self.memory[(self.memory_address as usize + y_line as usize) & 0xFFF];

                    for x_pixel in 0..8u16 {
                        let mask = 1 << (7 - x_pixel);
                        if data & mask != 0 {
                            let x = ((coord_x as u16 + x_pixel) % 64) as usize;
                            let y = ((coord_y as u16 + y_line as u16) % 32) as usize;

                            if self.screen_data[x][y] == 1 {
                                self.registers[0xF] = 1; //collision
                            }
                            self.screen_data[x][y] ^= 1;
                        }
                    }
                }
            }

            Operation::SkipIfKeyEquals(register) => {
                let key = (self.registers[register] & 0x0F) as usize;
                if self.keys[key] {
                    self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;
                }
            }

            Operation::SkipIfKeyNotEquals(register) => {
                let key = (self.registers[register] & 0x0F) as usize;
                if !self.keys[key] {
                    self.instruction_pointer = (self.instruction_pointer + 2) & 0x0FFF;
                }
            }

            Operation::SetToDelay(register) => {
                self.registers[register] = self.delay_timer;
            }

            Operation::SetToKey(register) => {
                if let Some(key) = self.keys.iter().position(|&value| value) {
                    self.registers[register] = key as u8;
                    self.waiting_for_key = None;
                } else {
                    self.waiting_for_key = Some(register)
                }
            }

            Operation::SetDelayTimer(register) => {
                self.delay_timer = self.registers[register];
            }

            Operation::SetSoundTimer(register) => {
                self.sound_timer = self.registers[register];
            }

            Operation::AddToI(register) => {
                self.memory_address =
                    (self.memory_address + self.registers[register] as u16) & 0xFFF;
            }

            Operation::SetIToSprite(register) => {
                let character = (self.registers[register] & 0x0F) as u16;
                self.memory_address = (Self::FONT_BASE + character * 5) & 0xFFF;
            }

            Operation::SetBinaryCodedDecimal(register) => {
                let value = self.registers[register];
                let memory_address = self.memory_address as usize & 0xFFF;

                self.memory[memory_address] = value / 100;
                self.memory[(memory_address + 1) & 0xFFF] = (value % 100) / 10;
                self.memory[(memory_address + 2) & 0xFFF] = value % 10;
            }

            Operation::RegisterDump(reg_x) => {
                for register in 0..=reg_x {
                    self.memory[self.memory_address as usize] = self.registers[register];
                    self.memory_address = (self.memory_address + 1) & 0x0FFF;
                }
            }

            Operation::RegisterLoad(reg_x) => {
                for register in 0..=reg_x {
                    self.registers[register] = self.memory[self.memory_address as usize];
                    self.memory_address = (self.memory_address + 1) & 0x0FFF;
                }
            }
        }
    }

    /// Update the `delay_timer` and `sound_timer` decrementing at a rate of 60Hz
    pub fn update_timers(&mut self) {
        self.update_timers_at(Instant::now());
    }

    /// Update the `delay_timer` and `sound_timer` decrementing at a rate of 60 Hz taking the instant
    /// that this function is called at
    ///
    /// # Arguments
    ///
    /// - `now` (`Instant`) - Instant of the call, used to ensure the 60 Hz rate
    pub fn update_timers_at(&mut self, now: Instant) {
        let mut elapsed = now.duration_since(self.last_timer_tick);

        while elapsed >= Self::TIMER_INTERVAL {
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }

            if self.sound_timer > 0 {
                self.sound_timer -= 1;
            }

            self.last_timer_tick += Self::TIMER_INTERVAL;
            elapsed -= Self::TIMER_INTERVAL;
        }
    }
}

impl Default for State {
    fn default() -> Self {
        let mut state = Self {
            memory: [0; 0x1000],
            registers: [0; 16],
            memory_address: 0,
            instruction_pointer: Self::INITIAL_MEMORY_ADDRESS,
            stack: Default::default(),
            screen_data: [[0; 32]; 64],
            delay_timer: 0,
            sound_timer: 0,
            last_timer_tick: Instant::now(),
            is_beeping: false,
            key_mappings: HashMap::from([
                (KeyCode::Key1, 0x1),
                (KeyCode::Key2, 0x2),
                (KeyCode::Key3, 0x3),
                (KeyCode::Key4, 0xC),
                (KeyCode::Q, 0x4),
                (KeyCode::W, 0x5),
                (KeyCode::E, 0x6),
                (KeyCode::R, 0xD),
                (KeyCode::A, 0x7),
                (KeyCode::S, 0x8),
                (KeyCode::D, 0x9),
                (KeyCode::F, 0xE),
                (KeyCode::Z, 0xA),
                (KeyCode::X, 0x0),
                (KeyCode::C, 0xB),
                (KeyCode::V, 0xF),
            ]),
            beeping_sound: None,
            waiting_for_key: None,
            keys: [false; 16],
            rng: rng(),
        };

        // load fonts
        state.memory[State::FONT_BASE as usize..State::FONT_BASE as usize + State::FONT.len()]
            .copy_from_slice(&State::FONT);

        state
    }
}

impl Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("registers", &self.registers)
            .field("memory_address", &self.memory_address)
            .field("instruction_pointer", &self.instruction_pointer)
            .field("stack", &self.stack)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ---------- helpers ----------

    fn state() -> State {
        // Most tests want a known PC; the default is 0x200, which is fine.
        State::default()
    }

    /// Execute a single op and return the state (for chaining).
    fn exec(s: &mut State, op: Operation) {
        s.perform_operation(op);
    }

    /// Set V0..V15 to the given values (panics if len != 16).
    fn set_regs(s: &mut State, vals: [u8; 16]) {
        s.registers = vals;
    }

    // ---------- 00E0 ClearDisplay ----------

    #[test]
    fn clear_display_zeroes_screen() {
        let mut s = state();
        s.screen_data = [[1; 32]; 64]; // fully lit
        exec(&mut s, Operation::ClearDisplay);
        assert_eq!(s.screen_data, [[0; 32]; 64]);
    }

    #[test]
    fn clear_display_idempotent() {
        let mut s = state();
        exec(&mut s, Operation::ClearDisplay);
        exec(&mut s, Operation::ClearDisplay);
        assert_eq!(s.screen_data, [[0; 32]; 64]);
    }

    // ---------- 00EE Return ----------

    #[test]
    fn return_pops_stack() {
        let mut s = state();
        s.stack.push(0x300);
        exec(&mut s, Operation::Return);
        assert_eq!(s.instruction_pointer, 0x300);
        assert!(s.stack.is_empty());
    }

    #[test]
    fn return_on_empty_stack_is_noop() {
        let mut s = state();
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::Return);
        assert_eq!(s.instruction_pointer, pc);
    }

    // ---------- 1NNN Goto ----------

    #[test]
    fn goto_sets_pc() {
        let mut s = state();
        exec(&mut s, Operation::Goto(0xABC));
        assert_eq!(s.instruction_pointer, 0xABC);
    }

    // ---------- 2NNN CallSubroutine ----------

    #[test]
    fn call_subroutine_pushes_pc() {
        let mut s = state();
        let pc_before = s.instruction_pointer;
        exec(&mut s, Operation::CallSubroutine(0x400));
        assert_eq!(s.stack, vec![pc_before]);
        assert_eq!(s.instruction_pointer, 0x400);
    }

    #[test]
    fn call_then_return_roundtrips() {
        let mut s = state();
        let pc_before = s.instruction_pointer;
        exec(&mut s, Operation::CallSubroutine(0x400));
        exec(&mut s, Operation::Goto(0x500)); // do work
        exec(&mut s, Operation::Return);
        assert_eq!(s.instruction_pointer, pc_before);
        assert!(s.stack.is_empty());
    }

    // ---------- 3XNN / 4XNN skips ----------

    #[test]
    fn skip_if_equals_skips_on_match() {
        let mut s = state();
        set_regs(&mut s, [0x42; 16]);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfEquals(3, 0x42));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    #[test]
    fn skip_if_equals_does_not_skip_on_mismatch() {
        let mut s = state();
        set_regs(&mut s, [0x42; 16]);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfEquals(3, 0x43));
        assert_eq!(s.instruction_pointer, pc);
    }

    #[test]
    fn skip_if_not_equals_skips_on_mismatch() {
        let mut s = state();
        set_regs(&mut s, [0x42; 16]);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfNotEquals(3, 0x43));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    #[test]
    fn skip_if_not_equals_does_not_skip_on_match() {
        let mut s = state();
        set_regs(&mut s, [0x42; 16]);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfNotEquals(3, 0x42));
        assert_eq!(s.instruction_pointer, pc);
    }

    // ---------- 5XY0 / 9XY0 register compares ----------

    #[test]
    fn skip_if_registers_equals() {
        let mut s = state();
        set_regs(&mut s, [0x11; 16]);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfRegistersEquals(2, 7));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    #[test]
    fn skip_if_registers_not_equals_skips_when_different() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x11;
        regs[7] = 0x22;
        set_regs(&mut s, regs);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfRegistersNotEquals(2, 7));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    #[test]
    fn skip_if_registers_not_equals_does_not_skip_when_equal() {
        let mut s = state();
        set_regs(&mut s, [0x11; 16]);
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfRegistersNotEquals(2, 7));
        assert_eq!(s.instruction_pointer, pc);
    }

    // ---------- 6XNN / 7XNN ----------

    #[test]
    fn assign_value_sets_register() {
        let mut s = state();
        exec(&mut s, Operation::AssignValue(5, 0xAB));
        assert_eq!(s.registers[5], 0xAB);
    }

    #[test]
    fn add_value_wraps_and_does_not_touch_vf() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[5] = 0xFF;
        regs[0xF] = 0x42; // sentinel — must not change
        set_regs(&mut s, regs);
        exec(&mut s, Operation::AddValue(5, 0x02));
        assert_eq!(s.registers[5], 0x01); // wrapped
        assert_eq!(s.registers[0xF], 0x42); // VF untouched
    }

    // ---------- 8XY0..8XY3 ----------

    #[test]
    fn assign_register_copies() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x99;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::AssignRegister(7, 2));
        assert_eq!(s.registers[7], 0x99);
    }

    #[test]
    fn bit_or_assign() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0b1010_0000;
        regs[7] = 0b0000_0101;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::BitOrAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b1010_0101);
    }

    #[test]
    fn bit_and_assign() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0b1010_1010;
        regs[7] = 0b1111_0000;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::BitAndAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b1010_0000);
    }

    #[test]
    fn bit_xor_assign() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0b1010_1010;
        regs[7] = 0b1111_0000;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::BitXorAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b0101_1010);
    }

    // ---------- 8XY4 Add ----------

    #[test]
    fn add_assign_no_overflow() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x10;
        regs[7] = 0x20;
        regs[0xF] = 0xFF; // sentinel
        set_regs(&mut s, regs);
        exec(&mut s, Operation::AddAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0x30);
        assert_eq!(s.registers[0xF], 0);
    }

    #[test]
    fn add_assign_overflow_wraps_and_sets_vf() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0xFF;
        regs[7] = 0x02;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::AddAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0x01);
        assert_eq!(s.registers[0xF], 1);
    }

    #[test]
    fn add_assign_into_vf_uses_result_then_flag() {
        // 8XY4 where X == F: result is written, then VF overwritten with the flag.
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[0xF] = 0xFF;
        regs[7] = 0x02;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::AddAssignRegister(0xF, 7));
        assert_eq!(s.registers[0xF], 1); // flag wins, not 0x01
    }

    // ---------- 8XY5 Sub ----------

    #[test]
    fn sub_no_borrow_sets_vf() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x30;
        regs[7] = 0x10;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::SubAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0x20);
        assert_eq!(s.registers[0xF], 1); // no borrow
    }

    #[test]
    fn sub_borrow_clears_vf_and_wraps() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x10;
        regs[7] = 0x30;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::SubAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0xE0); // wrapped
        assert_eq!(s.registers[0xF], 0); // borrow
    }

    // ---------- 8XY7 InvSub ----------

    #[test]
    fn inv_sub_no_borrow_sets_vf() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x10;
        regs[7] = 0x30;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::InvSubAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0x20); // VY - VX
        assert_eq!(s.registers[0xF], 1);
    }

    #[test]
    fn inv_sub_borrow_clears_vf() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0x30;
        regs[7] = 0x10;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::InvSubAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0xE0);
        assert_eq!(s.registers[0xF], 0);
    }

    // ---------- 8XY6 ShiftRight ----------

    #[test]
    fn shift_r_uses_vy_as_source_and_sets_vf_from_lsb() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0xFF; // VX — must be ignored (S-CHIP)
        regs[7] = 0b0000_0011; // VY
        set_regs(&mut s, regs);
        exec(&mut s, Operation::ShiftRAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b0000_0001);
        assert_eq!(s.registers[0xF], 1); // LSB of VY
    }

    #[test]
    fn shift_r_lsb_zero_sets_vf_zero() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[7] = 0b0000_0010;
        regs[0xF] = 0xFF; // sentinel
        set_regs(&mut s, regs);
        exec(&mut s, Operation::ShiftRAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b0000_0001);
        assert_eq!(s.registers[0xF], 0);
    }

    // ---------- 8XYE ShiftLeft ----------

    #[test]
    fn shift_l_uses_vy_as_source_and_sets_vf_from_msb() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[2] = 0xFF; // VX — must be ignored
        regs[7] = 0b1000_0001; // VY
        set_regs(&mut s, regs);
        exec(&mut s, Operation::ShiftLAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b0000_0010); // 0x81 << 1 wraps to 0x02
        assert_eq!(s.registers[0xF], 1); // MSB was set
    }

    #[test]
    fn shift_l_msb_zero_sets_vf_zero() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[7] = 0b0100_0000;
        regs[0xF] = 0xFF;
        set_regs(&mut s, regs);
        exec(&mut s, Operation::ShiftLAssignRegister(2, 7));
        assert_eq!(s.registers[2], 0b1000_0000);
        assert_eq!(s.registers[0xF], 0);
    }

    // ---------- ANNN SetI ----------

    #[test]
    fn set_i() {
        let mut s = state();
        exec(&mut s, Operation::SetI(0xABC));
        assert_eq!(s.memory_address, 0xABC);
    }

    // ---------- BXNN ----------
    #[test]
    fn goto_plus_vx_adds_vx() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[0] = 0x10;
        regs[1] = 0x20;
        regs[2] = 0x30;
        set_regs(&mut s, regs);

        exec(&mut s, Operation::GotoPlusVX(2, 0x300));
        assert_eq!(s.instruction_pointer, 0x330); // 0x300 + V2
    }

    #[test]
    fn goto_plus_vx_uses_correct_register() {
        let mut s = state();
        let mut regs = [0u8; 16];
        regs[0] = 0xFF; // V0 is a red herring — must NOT be used
        regs[5] = 0x10;
        set_regs(&mut s, regs);

        exec(&mut s, Operation::GotoPlusVX(5, 0x200));
        assert_eq!(s.instruction_pointer, 0x210); // 0x200 + V5, not V0
    }

    #[test]
    fn goto_plus_vx_masks_to_12_bits() {
        let mut s = state();
        s.registers[3] = 0xFF;
        exec(&mut s, Operation::GotoPlusVX(3, 0xFFF));
        // 0xFFF + 0xFF = 0x10FE → 0x0FE
        assert_eq!(s.instruction_pointer, 0x0FE);
    }

    #[test]
    fn goto_plus_vx_does_not_touch_vf() {
        let mut s = state();
        s.registers[3] = 0x10;
        s.registers[0xF] = 0x42;
        exec(&mut s, Operation::GotoPlusVX(3, 0x300));
        assert_eq!(s.registers[0xF], 0x42);
    }

    // ---------- CXNN Random ----------

    #[test]
    fn random_result_is_masked_with_nn() {
        // Deterministic check: with NN = 0x00, result must be 0 regardless of RNG.
        let mut s = state();
        for _ in 0..100 {
            exec(&mut s, Operation::Random(3, 0x00));
            assert_eq!(s.registers[3], 0);
        }
    }

    #[test]
    fn random_result_within_nn_bits() {
        let mut s = state();
        for _ in 0..100 {
            exec(&mut s, Operation::Random(3, 0x0F));
            assert!(s.registers[3] <= 0x0F);
        }
    }

    // ---------- DXYN Draw ----------

    #[test]
    fn draw_single_pixel_sets_it() {
        let mut s = state();
        s.memory_address = 0x200;
        s.memory[0x200] = 0b1000_0000; // one pixel at x offset 0
        s.registers[0] = 0; // VX
        s.registers[1] = 0; // VY
        exec(&mut s, Operation::Draw(0, 1, 1));
        assert_eq!(s.screen_data[0][0], 1);
        assert_eq!(s.registers[0xF], 0); // no collision
    }

    #[test]
    fn draw_twice_collides_and_clears() {
        let mut s = state();
        s.memory_address = 0x200;
        s.memory[0x200] = 0b1000_0000;
        s.registers[0] = 0;
        s.registers[1] = 0;
        exec(&mut s, Operation::Draw(0, 1, 1));
        assert_eq!(s.screen_data[0][0], 1);
        exec(&mut s, Operation::Draw(0, 1, 1));
        assert_eq!(s.screen_data[0][0], 0); // XOR'd off
        assert_eq!(s.registers[0xF], 1); // collision
    }

    #[test]
    fn draw_wraps_at_edges() {
        // Sprite at x = 63, one pixel at x offset 0 → wraps to x = 63.
        let mut s = state();
        s.memory_address = 0x200;
        s.memory[0x200] = 0b1000_0000;
        s.registers[0] = 63;
        s.registers[1] = 0;
        exec(&mut s, Operation::Draw(0, 1, 1));
        assert_eq!(s.screen_data[63][0], 1);
    }

    #[test]
    fn draw_wraps_right_edge_to_left() {
        // Sprite at x = 63, pixel at x offset 1 → wraps to x = 0.
        let mut s = state();
        s.memory_address = 0x200;
        s.memory[0x200] = 0b0100_0000; // pixel at x offset 1
        s.registers[0] = 63;
        s.registers[1] = 0;
        exec(&mut s, Operation::Draw(0, 1, 1));
        assert_eq!(s.screen_data[0][0], 1);
    }

    // ---------- EX9E / EXA1 key skips ----------

    #[test]
    fn skip_if_key_equals_skips_when_pressed() {
        let mut s = state();
        s.registers[3] = 0x5;
        s.keys[5] = true;
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfKeyEquals(3));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    #[test]
    fn skip_if_key_equals_uses_low_nibble() {
        let mut s = state();
        s.registers[3] = 0xA5; // low nibble = 5
        s.keys[5] = true;
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfKeyEquals(3));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    #[test]
    fn skip_if_key_not_equals_skips_when_released() {
        let mut s = state();
        s.registers[3] = 0x5;
        s.keys[5] = false;
        let pc = s.instruction_pointer;
        exec(&mut s, Operation::SkipIfKeyNotEquals(3));
        assert_eq!(s.instruction_pointer, pc + 2);
    }

    // ---------- FX07 / FX15 / FX18 timers ----------

    #[test]
    fn set_to_delay_reads_timer() {
        let mut s = state();
        s.delay_timer = 0x42;
        exec(&mut s, Operation::SetToDelay(3));
        assert_eq!(s.registers[3], 0x42);
    }

    #[test]
    fn set_delay_timer_writes_timer() {
        let mut s = state();
        s.registers[3] = 0x99;
        exec(&mut s, Operation::SetDelayTimer(3));
        assert_eq!(s.delay_timer, 0x99);
    }

    #[test]
    fn set_sound_timer_writes_timer() {
        let mut s = state();
        s.registers[3] = 0x77;
        exec(&mut s, Operation::SetSoundTimer(3));
        assert_eq!(s.sound_timer, 0x77);
    }

    // ---------- FX0A SetToKey ----------

    #[test]
    fn set_to_key_blocks_when_no_key() {
        let mut s = state();
        exec(&mut s, Operation::SetToKey(3));
        assert_eq!(s.waiting_for_key, Some(3));
        assert_eq!(s.registers[3], 0); // untouched
    }

    #[test]
    fn set_to_key_stores_first_pressed_key() {
        let mut s = state();
        s.keys[7] = true;
        exec(&mut s, Operation::SetToKey(3));
        assert_eq!(s.registers[3], 7);
        assert_eq!(s.waiting_for_key, None);
    }

    #[test]
    fn set_to_key_clears_waiting_after_key_press() {
        let mut s = state();
        exec(&mut s, Operation::SetToKey(3)); // blocks
        assert_eq!(s.waiting_for_key, Some(3));
        s.keys[2] = true; // user presses key 2
        // Re-injected op runs again (simulating get_next_operation's waiting path)
        exec(&mut s, Operation::SetToKey(3));
        assert_eq!(s.registers[3], 2);
        assert_eq!(s.waiting_for_key, None);
    }

    // ---------- FX1E AddToI ----------

    #[test]
    fn add_to_i_adds_register() {
        let mut s = state();
        s.memory_address = 0x200;
        s.registers[3] = 0x10;
        exec(&mut s, Operation::AddToI(3));
        assert_eq!(s.memory_address, 0x210);
    }

    #[test]
    fn add_to_i_does_not_touch_vf() {
        let mut s = state();
        s.memory_address = 0x200;
        s.registers[3] = 0x10;
        s.registers[0xF] = 0x42;
        exec(&mut s, Operation::AddToI(3));
        assert_eq!(s.registers[0xF], 0x42);
    }

    // ---------- FX29 SetIToSprite ----------

    #[test]
    fn set_i_to_sprite_points_at_font_glyph() {
        let mut s = state();
        s.registers[3] = 0x5;
        exec(&mut s, Operation::SetIToSprite(3));
        assert_eq!(s.memory_address, State::FONT_BASE + 5 * 5);
    }

    #[test]
    fn set_i_to_sprite_uses_low_nibble() {
        let mut s = state();
        s.registers[3] = 0xA5; // low nibble = 5
        exec(&mut s, Operation::SetIToSprite(3));
        assert_eq!(s.memory_address, State::FONT_BASE + 5 * 5);
    }

    // ---------- FX33 BCD ----------

    #[test]
    fn bcd_writes_three_digits() {
        let mut s = state();
        s.memory_address = 0x300;
        s.registers[3] = 254;
        exec(&mut s, Operation::SetBinaryCodedDecimal(3));
        assert_eq!(s.memory[0x300], 2);
        assert_eq!(s.memory[0x301], 5);
        assert_eq!(s.memory[0x302], 4);
    }

    #[test]
    fn bcd_zero() {
        let mut s = state();
        s.memory_address = 0x300;
        s.registers[3] = 0;
        exec(&mut s, Operation::SetBinaryCodedDecimal(3));
        assert_eq!(&s.memory[0x300..=0x302], &[0, 0, 0]);
    }

    #[test]
    fn bcd_255() {
        let mut s = state();
        s.memory_address = 0x300;
        s.registers[3] = 255;
        exec(&mut s, Operation::SetBinaryCodedDecimal(3));
        assert_eq!(&s.memory[0x300..=0x302], &[2, 5, 5]);
    }

    #[test]
    fn bcd_wraps_at_end_of_memory() {
        // I = 0xFFE, writes to 0xFFE, 0xFFF, 0x000 (wrap)
        let mut s = state();
        s.memory_address = 0xFFE;
        s.registers[3] = 123;
        exec(&mut s, Operation::SetBinaryCodedDecimal(3));
        assert_eq!(s.memory[0xFFE], 1);
        assert_eq!(s.memory[0xFFF], 2);
        assert_eq!(s.memory[0x000], 3);
    }

    // ---------- FX55 / FX65 ----------

    #[test]
    fn register_dump_writes_and_advances_i() {
        let mut s = state();
        s.memory_address = 0x300;
        let mut regs = [0u8; 16];
        for (i, reg) in regs[..4].iter_mut().enumerate() {
            *reg = i as u8 + 1;
        }
        set_regs(&mut s, regs);
        exec(&mut s, Operation::RegisterDump(3));
        assert_eq!(&s.memory[0x300..=0x303], &[1, 2, 3, 4]);
        assert_eq!(s.memory_address, 0x304); // I advanced (S-CHIP)
    }

    #[test]
    fn register_load_reads_and_advances_i() {
        let mut s = state();
        s.memory_address = 0x300;
        s.memory[0x300..=0x303].copy_from_slice(&[9, 8, 7, 6]);
        exec(&mut s, Operation::RegisterLoad(3));
        assert_eq!(&s.registers[0..=3], &[9, 8, 7, 6]);
        assert_eq!(s.memory_address, 0x304);
    }

    #[test]
    fn register_dump_wraps_at_end_of_memory() {
        let mut s = state();
        s.memory_address = 0xFFF;
        s.registers[0] = 0xAA;
        s.registers[1] = 0xBB;
        exec(&mut s, Operation::RegisterDump(1));
        assert_eq!(s.memory[0xFFF], 0xAA);
        assert_eq!(s.memory[0x000], 0xBB);
        assert_eq!(s.memory_address, 0x001);
    }

    // ---------- update_timers ----------

    #[test]
    fn timers_decrement_at_60hz() {
        let mut s = state();
        s.delay_timer = 10;
        s.sound_timer = 10;
        let base = s.last_timer_tick;
        // Advance 3 full ticks (3 * 1/60 s)
        s.update_timers_at(base + Duration::from_secs_f64(3.0 / 60.0));
        assert_eq!(s.delay_timer, 7);
        assert_eq!(s.sound_timer, 7);
    }

    #[test]
    fn timers_do_not_underflow() {
        let mut s = state();
        s.delay_timer = 1;
        s.sound_timer = 1;
        let base = s.last_timer_tick;
        s.update_timers_at(base + Duration::from_secs(10)); // way more than needed
        assert_eq!(s.delay_timer, 0);
        assert_eq!(s.sound_timer, 0);
    }

    #[test]
    fn timers_catch_up_after_long_pause() {
        let mut s = state();
        s.delay_timer = 100;
        let base = s.last_timer_tick;
        // 1 full second = 60 ticks
        s.update_timers_at(base + Duration::from_secs(1));
        assert_eq!(s.delay_timer, 40);
    }

    #[test]
    fn timers_no_decrement_before_one_tick() {
        let mut s = state();
        s.delay_timer = 10;
        let base = s.last_timer_tick;
        s.update_timers_at(base + Duration::from_millis(1));
        assert_eq!(s.delay_timer, 10);
    }

    // ---------- beep state machine ----------

    #[test]
    fn beep_starts_when_sound_timer_positive() {
        let mut s = state();
        s.sound_timer = 5;
        s.beep();
        assert!(s.is_beeping);
    }

    #[test]
    fn beep_stops_when_sound_timer_zero() {
        let mut s = state();
        s.sound_timer = 5;
        s.beep();
        s.sound_timer = 0;
        s.beep();
        assert!(!s.is_beeping);
    }

    #[test]
    fn beep_idempotent_while_playing() {
        let mut s = state();
        s.sound_timer = 5;
        s.beep();
        s.beep(); // second call should be a no-op
        assert!(s.is_beeping);
    }
}
