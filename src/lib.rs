use std::{fs, io::Read};

#[derive(Debug)]
pub struct State {
    memory: [u8; 0xFFF],         // Chip 8 computers had commonly 4 KB memories
    registers: [u8; 16],         // 16 1-byte registers labeled from V0 to VF
    address: u16,                // 1 2-byte register labeled as I
    counter: u16,                // 2-byte program counter
    stack: Vec<u16>,             // 16-bit stack
    screen_data: [[u8; 64]; 32], // 64 x 32 screen "bytes"
}

impl State {
    pub fn load_game(&mut self, path: &str) -> std::io::Result<usize> {
        let mut file = fs::File::open(path)?;
        file.read(&mut self.memory[0x200..])
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            memory: [0; 0xFFF],
            registers: [0; 16],
            address: 0,
            // Initial address is set at 0x200 since in the original chip 8 computers, the emulator took the first
            // 0x200 bytes. For modern implementations, there is no need to do this and the counter could start at 0x000
            counter: 0x200,
            stack: Default::default(),
            screen_data: [[0; 64]; 32],
        }
    }
}
