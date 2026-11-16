use std::{
    collections::HashMap,
    fmt::Debug,
    fs,
    io::Read,
    time::{Duration, Instant},
};

use macroquad::{
    audio::{PlaySoundParams, Sound, play_sound, stop_sound},
    input::KeyCode,
};
use rand::{RngExt, rng, rngs::ThreadRng};

use crate::operation::Operation;

pub struct State {
    /// 4 KB memory representation
    memory: [u8; 0xFFF],
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
    pub sound_timer: u8,
    /// Timer of the last timer decrement, used to make the timers work in 60 Hz
    last_timer_tick: std::time::Instant,
    /// Sound to play when it's supposed to be beeping
    beeping_sound: Sound,
    /// Flag to know if the game is beeping
    is_beeping: bool,
    /// Key mappings from keyboard code to CHIP 8 key
    pub key_mappings: HashMap<KeyCode, usize>,
    /// Flag to know if the CPU is waiting for a key, used to avoid thread blocking. The `usize` is the
    /// register number where the key value must be stored
    waiting_for_key: Option<usize>,
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
    const TIMER_INTERVAL: f64 = 1.0 / 60.0;

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

    pub fn new(beeping_sound: Sound) -> Self {
        let mut state = Self {
            memory: [0; 0xFFF],
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
            beeping_sound,
            waiting_for_key: None,
            keys: [false; 16],
            rng: rng(),
        };

        // load fonts
        state.memory[State::FONT_BASE as usize..State::FONT_BASE as usize + State::FONT.len()]
            .copy_from_slice(&State::FONT);

        state
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
        let mut file = fs::File::open(path)?;
        file.read(&mut self.memory[Self::INITIAL_MEMORY_ADDRESS as usize..])
    }

    pub fn beep(&mut self) {
        if self.sound_timer > 0 && !self.is_beeping {
            play_sound(
                &self.beeping_sound,
                PlaySoundParams {
                    looped: true,
                    volume: 1.0,
                },
            );
            self.is_beeping = true;
        } else if self.sound_timer == 0 && self.is_beeping {
            stop_sound(&self.beeping_sound);
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
            let opcode = ((self.memory[counter] as u16) << 8u16) | self.memory[counter + 1] as u16;
            self.instruction_pointer += 2;

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
                    self.instruction_pointer += 2;
                }
            }

            Operation::SkipIfNotEquals(register, value) => {
                if self.registers[register] != value {
                    self.instruction_pointer += 2;
                }
            }

            Operation::SkipIfRegistersEquals(reg_x, reg_y) => {
                if self.registers[reg_x] == self.registers[reg_y] {
                    self.instruction_pointer += 2;
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
                    self.instruction_pointer += 2;
                }
            }

            Operation::SetI(value) => self.memory_address = value,

            Operation::GotoPlusV0(address) => {
                self.instruction_pointer = address + self.registers[0x0] as u16;
            }

            Operation::Random(register, value) => {
                self.registers[register] = self.rng.random_range(0..=0xFF) & value
            }

            Operation::Draw(reg_x, reg_y, height) => {
                let coord_x = self.registers[reg_x];
                let coord_y = self.registers[reg_y];

                self.registers[0xf] = 0;

                // loop for the amount of vertical lines needed to draw
                for y_line in 0..height {
                    let data = self.memory[(self.memory_address + y_line as u16) as usize];
                    let mut x_pixel_inv = 7;

                    for x_pixel in 0..8 {
                        let mask = 1 << x_pixel_inv;
                        if data & mask != 0 {
                            let x = (coord_x + x_pixel).min(63) as usize;
                            let y = (coord_y + y_line).min(31) as usize;

                            if self.screen_data[x][y] == 1 {
                                self.registers[0xF] = 1; //collision
                            }
                            self.screen_data[x][y] ^= 1;
                        }
                        x_pixel_inv -= 1;
                    }
                }
            }

            Operation::SkipIfKeyEquals(register) => {
                let key = (self.registers[register] & 0x0F) as usize;
                if self.keys[key] {
                    self.instruction_pointer += 2;
                }
            }

            Operation::SkipIfKeyNotEquals(register) => {
                let key = (self.registers[register] & 0x0F) as usize;
                if !self.keys[key] {
                    self.instruction_pointer += 2;
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
                self.memory_address = self
                    .memory_address
                    .wrapping_add(self.registers[register] as u16);
            }

            Operation::SetIToSprite(register) => {
                let character = (self.registers[register] & 0x0F) as u16;
                self.memory_address = Self::FONT_BASE + character * 5;
            }

            Operation::SetBinaryCodedDecimal(register) => {
                let value = self.registers[register];
                let memory_address = self.memory_address as usize;

                let mem_len = self.memory.len();
                self.memory[memory_address % mem_len] = value / 100;
                self.memory[(memory_address + 1) % mem_len] = (value % 100) / 10;
                self.memory[(memory_address + 2) % mem_len] = value % 10;
            }

            Operation::RegisterDump(reg_x) => {
                let len = self.memory.len();
                for register in 0..=reg_x {
                    self.memory[(self.memory_address as usize) % len] = self.registers[register];
                    self.memory_address = self.memory_address.wrapping_add(1);
                }
            }

            Operation::RegisterLoad(reg_x) => {
                let len = self.memory.len();
                for register in 0..=reg_x {
                    self.registers[register] = self.memory[(self.memory_address as usize) % len];
                    self.memory_address = self.memory_address.wrapping_add(1);
                }
            }
        }
    }

    pub fn update_timers(&mut self) {
        let now = Instant::now();
        let mut elapsed = now.duration_since(self.last_timer_tick).as_secs_f64();

        while elapsed >= Self::TIMER_INTERVAL {
            if self.delay_timer > 0 {
                self.delay_timer -= 1;
            }

            if self.sound_timer > 0 {
                self.sound_timer -= 1;
            }

            self.last_timer_tick += Duration::from_secs_f64(Self::TIMER_INTERVAL);
            elapsed -= Self::TIMER_INTERVAL;
        }
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
