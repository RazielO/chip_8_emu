/// Valid Chip 8 operations
///
/// # Conventions
///
/// - `usize` denotes a register index in `0..=0xF` (i.e. `V0`–`VF`).
/// - `u16` in an address position is a 12-bit address (`0x000`–`0xFFF`).
/// - `u8` in an immediate position is an 8-bit immediate (`0x00`–`0xFF`).
/// - `u8` is used for the sprite-height nibble `N` in `DXYN` (`0x0`–`0xF`).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Operation {
    /// Opcode `0NNN`. Calls the machine-code routine at address `NNN`.
    CallAssemblySubroutine(u16),
    /// Opcode `00E0`. Clears the screen.
    ClearDisplay,
    /// Opcode `00EE`. Returns from a subroutine.
    Return,
    /// Opcode `1NNN`. Jumps to address `NNN`.
    Goto(u16),
    /// Opcode `2NNN`. Calls the subroutine at address `NNN`.
    CallSubroutine(u16),
    /// Opcode `3XNN`. Skips the next instruction if `VX == NN`.
    SkipIfEquals(usize, u8),
    /// Opcode `4XNN`. Skips the next instruction if `VX != NN`.
    SkipIfNotEquals(usize, u8),
    /// Opcode `5XY0`. Skips the next instruction if `VX == VY`.
    SkipIfRegistersEquals(usize, usize),
    /// Opcode `6XNN`. Sets `VX = NN`.
    AssignValue(usize, u8),
    /// Opcode `7XNN`. Sets `VX += NN`. The carry flag (`VF`) is **not** changed.
    AddValue(usize, u8),
    /// Opcode `8XY0`. Sets `VX = VY`.
    AssignRegister(usize, usize),
    /// Opcode `8XY1`. Sets `VX = VX | VY`.
    BitOrAssignRegister(usize, usize),
    /// Opcode `8XY2`. Sets `VX = VX & VY`.
    BitAndAssignRegister(usize, usize),
    /// Opcode `8XY3`. Sets `VX = VX ^ VY`.
    BitXorAssignRegister(usize, usize),
    /// Opcode `8XY4`. Sets `VX += VY`. `VF` is set to `1` on overflow, `0` otherwise.
    AddAssignRegister(usize, usize),
    /// Opcode `8XY5`. Sets `VX -= VY`. `VF` is set to `1` if `VX >= VY`, `0` otherwise.
    SubAssignRegister(usize, usize),
    /// Opcode `8XY6`. Shifts `VX` right by 1; `VF` receives the bit shifted out.
    ShiftRAssignRegister(usize, usize),
    /// Opcode `8XY7`. Sets `VX = VY - VX`. `VF` is set to `1` if `VY >= VX`, `0` otherwise.
    InvSubAssignRegister(usize, usize),
    /// Opcode `8XYE`. Shifts `VX` left by 1; `VF` receives the bit shifted out.
    ShiftLAssignRegister(usize, usize),
    /// Opcode `9XY0`. Skips the next instruction if `VX != VY`.
    SkipIfRegistersNotEquals(usize, usize),
    /// Opcode `ANNN`. Sets the index register `I = NNN`.
    SetI(u16),
    /// Opcode `BNNN`. Jumps to `NNN + V0`.
    GotoPlusV0(u16),
    /// Opcode `CXNN`. Sets `VX = rand() & NN`.
    Random(usize, u8),
    /// Opcode `DXYN`. Draws an 8×N sprite at `(VX, VY)` from memory at `I`. `VF` is set to `1` if any lit pixel is erased (collision), `0` otherwise.
    Draw(usize, usize, u8),
    /// Opcode `EX9E`. Skips the next instruction if the key in `VX` (low nibble) is pressed.
    SkipIfKeyEquals(usize),
    /// Opcode `EXA1`. Skips the next instruction if the key in `VX` (low nibble) is not pressed.
    SkipIfKeyNotEquals(usize),
    /// Opcode `FX07`. Sets `VX` to the current delay-timer value.
    SetToDelay(usize),
    /// Opcode `FX0A`. Blocks until a key is pressed, then stores it in `VX`.
    SetToKey(usize),
    /// Opcode `FX15`. Sets the delay timer to `VX`.
    SetDelayTimer(usize),
    /// Opcode `FX18`. Sets the sound timer to `VX`.
    SetSoundTimer(usize),
    /// Opcode `FX1E`. Sets `I += VX`. `VF` is **not** affected.
    AddToI(usize),
    /// Opcode `FX29`. Sets `I` to the address of the 4×5 hex font sprite for the character in `VX` (low nibble).
    SetIToSprite(usize),
    /// Opcode `FX33`. Writes the BCD digits of `VX` (hundreds, tens, ones) to `I`, `I+1`, `I+2` respectively. `I` is unchanged.
    SetBinaryCodedDecimal(usize),
    /// Opcode `FX55`. Writes `V0..=VX` to memory starting at `I`. `I` itself is
    ///   left unmodified (per the `[d]` note in the reference table; the original interpreter incremented `I`).
    RegisterDump(usize),
    /// Opcode `FX65`. Reads `V0..=VX` from memory starting at `I`. `I` itself is
    ///   left unmodified (same `[d]` caveat as `RegisterDump`).
    RegisterLoad(usize),
}

impl TryFrom<u16> for Operation {
    type Error = String;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let nnn = value & 0x0FFF; // NNN address (last 3 bytes)
        let x = ((value & 0x0F00) >> 8) as usize; // X register (2nd byte)
        let y = ((value & 0x00F0) >> 4) as usize; // Y register (3rd byte)
        let n = (value & 0x000F) as u8; // 4th byte
        let nn = (value & 0x00FF) as u8; // NN value (last 2 bytes)

        match (value & 0xF000) >> 12 {
            0x0 if value == 0x00E0 => Ok(Self::ClearDisplay),
            0x0 if value == 0x00EE => Ok(Self::Return),
            0x0 => Ok(Self::CallAssemblySubroutine(nnn)),
            0x1 => Ok(Self::Goto(nnn)),
            0x2 => Ok(Self::CallSubroutine(nnn)),
            0x3 => Ok(Self::SkipIfEquals(x, nn)),
            0x4 => Ok(Self::SkipIfNotEquals(x, nn)),
            0x5 if n == 0x0 => Ok(Self::SkipIfRegistersEquals(x, y)),
            0x6 => Ok(Self::AssignValue(x, nn)),
            0x7 => Ok(Self::AddValue(x, nn)),
            0x8 if n == 0x0 => Ok(Self::AssignRegister(x, y)),
            0x8 if n == 0x1 => Ok(Self::BitOrAssignRegister(x, y)),
            0x8 if n == 0x2 => Ok(Self::BitAndAssignRegister(x, y)),
            0x8 if n == 0x3 => Ok(Self::BitXorAssignRegister(x, y)),
            0x8 if n == 0x4 => Ok(Self::AddAssignRegister(x, y)),
            0x8 if n == 0x5 => Ok(Self::SubAssignRegister(x, y)),
            0x8 if n == 0x6 => Ok(Self::ShiftRAssignRegister(x, y)),
            0x8 if n == 0x7 => Ok(Self::InvSubAssignRegister(x, y)),
            0x8 if n == 0xE => Ok(Self::ShiftLAssignRegister(x, y)),
            0x9 if n == 0x0 => Ok(Self::SkipIfRegistersNotEquals(x, y)),
            0xA => Ok(Self::SetI(nnn)),
            0xB => Ok(Self::GotoPlusV0(nnn)),
            0xC => Ok(Self::Random(x, nn)),
            0xD => Ok(Self::Draw(x, y, n)),
            0xE if nn == 0x9E => Ok(Self::SkipIfKeyEquals(x)),
            0xE if nn == 0xA1 => Ok(Self::SkipIfKeyNotEquals(x)),
            0xF if nn == 0x07 => Ok(Self::SetToDelay(x)),
            0xF if nn == 0x0A => Ok(Self::SetToKey(x)),
            0xF if nn == 0x15 => Ok(Self::SetDelayTimer(x)),
            0xF if nn == 0x18 => Ok(Self::SetSoundTimer(x)),
            0xF if nn == 0x1E => Ok(Self::AddToI(x)),
            0xF if nn == 0x29 => Ok(Self::SetIToSprite(x)),
            0xF if nn == 0x33 => Ok(Self::SetBinaryCodedDecimal(x)),
            0xF if nn == 0x55 => Ok(Self::RegisterDump(x)),
            0xF if nn == 0x65 => Ok(Self::RegisterLoad(x)),
            _ => Err(format!("Invalid opcode 0x{value:04X}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to reduce noise
    fn decode(w: u16) -> Result<Operation, String> {
        Operation::try_from(w)
    }

    // ---------- 0NNN family ----------
    #[test]
    fn call_assembly_subroutine() {
        assert_eq!(
            decode(0x0ABC),
            Ok(Operation::CallAssemblySubroutine(0x0ABC))
        );
        assert_eq!(
            decode(0x0000),
            Ok(Operation::CallAssemblySubroutine(0x0000))
        );
        assert_eq!(
            decode(0x0FFF),
            Ok(Operation::CallAssemblySubroutine(0x0FFF))
        );
    }

    #[test]
    fn clear_display() {
        assert_eq!(decode(0x00E0), Ok(Operation::ClearDisplay));
    }

    #[test]
    fn return_from_subroutine() {
        assert_eq!(decode(0x00EE), Ok(Operation::Return));
    }

    // ---------- 1NNN ----------
    #[test]
    fn goto() {
        assert_eq!(decode(0x1ABC), Ok(Operation::Goto(0x0ABC)));
        assert_eq!(decode(0x1000), Ok(Operation::Goto(0x0000)));
        assert_eq!(decode(0x1FFF), Ok(Operation::Goto(0x0FFF)));
    }

    // ---------- 2NNN ----------
    #[test]
    fn call_subroutine() {
        assert_eq!(decode(0x2ABC), Ok(Operation::CallSubroutine(0x0ABC)));
        assert_eq!(decode(0x2345), Ok(Operation::CallSubroutine(0x0345)));
        assert_eq!(decode(0x2FFF), Ok(Operation::CallSubroutine(0x0FFF)));
    }

    // ---------- 3XNN ----------
    #[test]
    fn skip_if_equals() {
        assert_eq!(decode(0x3A12), Ok(Operation::SkipIfEquals(0xA, 0x12)));
        assert_eq!(decode(0x3000), Ok(Operation::SkipIfEquals(0x0, 0x00)));
        assert_eq!(decode(0x3FFF), Ok(Operation::SkipIfEquals(0xF, 0xFF)));
    }

    // ---------- 4XNN ----------
    #[test]
    fn skip_if_not_equals() {
        assert_eq!(decode(0x4B34), Ok(Operation::SkipIfNotEquals(0xB, 0x34)));
        assert_eq!(decode(0x4000), Ok(Operation::SkipIfNotEquals(0x0, 0x00)));
        assert_eq!(decode(0x4FAB), Ok(Operation::SkipIfNotEquals(0xF, 0xAB)));
    }

    // ---------- 5XY0 ----------
    #[test]
    fn skip_if_registers_equals() {
        assert_eq!(
            decode(0x5AB0),
            Ok(Operation::SkipIfRegistersEquals(0xA, 0xB))
        );
        assert_eq!(
            decode(0x5000),
            Ok(Operation::SkipIfRegistersEquals(0x0, 0x0))
        );
        assert_eq!(
            decode(0x5F10),
            Ok(Operation::SkipIfRegistersEquals(0xF, 0x1))
        );
    }

    // ---------- 6XNN ----------
    #[test]
    fn assign_value() {
        assert_eq!(decode(0x6A42), Ok(Operation::AssignValue(0xA, 0x42)));
        assert_eq!(decode(0x6000), Ok(Operation::AssignValue(0x0, 0x00)));
        assert_eq!(decode(0x6FFF), Ok(Operation::AssignValue(0xF, 0xFF)));
    }

    // ---------- 7XNN ----------
    #[test]
    fn add_value() {
        assert_eq!(decode(0x7B10), Ok(Operation::AddValue(0xB, 0x10)));
        assert_eq!(decode(0x7000), Ok(Operation::AddValue(0x0, 0x00)));
        assert_eq!(decode(0x7FFF), Ok(Operation::AddValue(0xF, 0xFF)));
    }

    // ---------- 8XY0 ----------
    #[test]
    fn assign_register() {
        assert_eq!(decode(0x8AB0), Ok(Operation::AssignRegister(0xA, 0xB)));
        assert_eq!(decode(0x8000), Ok(Operation::AssignRegister(0x0, 0x0)));
        assert_eq!(decode(0x8F10), Ok(Operation::AssignRegister(0xF, 0x1)));
    }

    // ---------- 8XY1 ----------
    #[test]
    fn bit_or_assign_register() {
        assert_eq!(decode(0x8AB1), Ok(Operation::BitOrAssignRegister(0xA, 0xB)));
        assert_eq!(decode(0x8011), Ok(Operation::BitOrAssignRegister(0x0, 0x1)));
        assert_eq!(decode(0x8F11), Ok(Operation::BitOrAssignRegister(0xF, 0x1)));
    }

    // ---------- 8XY2 ----------
    #[test]
    fn bit_and_assign_register() {
        assert_eq!(
            decode(0x8AB2),
            Ok(Operation::BitAndAssignRegister(0xA, 0xB))
        );
        assert_eq!(
            decode(0x8022),
            Ok(Operation::BitAndAssignRegister(0x0, 0x2))
        );
        assert_eq!(
            decode(0x8F22),
            Ok(Operation::BitAndAssignRegister(0xF, 0x2))
        );
    }

    // ---------- 8XY3 ----------
    #[test]
    fn bit_xor_assign_register() {
        assert_eq!(
            decode(0x8AB3),
            Ok(Operation::BitXorAssignRegister(0xA, 0xB))
        );
        assert_eq!(
            decode(0x8033),
            Ok(Operation::BitXorAssignRegister(0x0, 0x3))
        );
        assert_eq!(
            decode(0x8F33),
            Ok(Operation::BitXorAssignRegister(0xF, 0x3))
        );
    }

    // ---------- 8XY4 ----------
    #[test]
    fn add_assign_register() {
        assert_eq!(decode(0x8AB4), Ok(Operation::AddAssignRegister(0xA, 0xB)));
        assert_eq!(decode(0x8044), Ok(Operation::AddAssignRegister(0x0, 0x4)));
        assert_eq!(decode(0x8F44), Ok(Operation::AddAssignRegister(0xF, 0x4)));
    }

    // ---------- 8XY5 ----------
    #[test]
    fn sub_assign_register() {
        assert_eq!(decode(0x8AB5), Ok(Operation::SubAssignRegister(0xA, 0xB)));
        assert_eq!(decode(0x8055), Ok(Operation::SubAssignRegister(0x0, 0x5)));
        assert_eq!(decode(0x8F55), Ok(Operation::SubAssignRegister(0xF, 0x5)));
    }

    // ---------- 8XY6 ----------
    #[test]
    fn shift_r_assign_register() {
        assert_eq!(
            decode(0x8AB6),
            Ok(Operation::ShiftRAssignRegister(0xA, 0xB))
        );
        assert_eq!(
            decode(0x8066),
            Ok(Operation::ShiftRAssignRegister(0x0, 0x6))
        );
        assert_eq!(
            decode(0x8F66),
            Ok(Operation::ShiftRAssignRegister(0xF, 0x6))
        );
    }

    // ---------- 8XY7 ----------
    #[test]
    fn inv_sub_assign_register() {
        assert_eq!(
            decode(0x8AB7),
            Ok(Operation::InvSubAssignRegister(0xA, 0xB))
        );
        assert_eq!(
            decode(0x8077),
            Ok(Operation::InvSubAssignRegister(0x0, 0x7))
        );
        assert_eq!(
            decode(0x8F77),
            Ok(Operation::InvSubAssignRegister(0xF, 0x7))
        );
    }

    // ---------- 8XYE ----------
    #[test]
    fn shift_l_assign_register() {
        assert_eq!(
            decode(0x8ABE),
            Ok(Operation::ShiftLAssignRegister(0xA, 0xB))
        );
        assert_eq!(
            decode(0x80EE),
            Ok(Operation::ShiftLAssignRegister(0x0, 0xE))
        );
        assert_eq!(
            decode(0x8FEE),
            Ok(Operation::ShiftLAssignRegister(0xF, 0xE))
        );
    }

    // ---------- 9XY0 ----------
    #[test]
    fn skip_if_registers_not_equals() {
        assert_eq!(
            decode(0x9AB0),
            Ok(Operation::SkipIfRegistersNotEquals(0xA, 0xB))
        );
        assert_eq!(
            decode(0x9000),
            Ok(Operation::SkipIfRegistersNotEquals(0x0, 0x0))
        );
        assert_eq!(
            decode(0x9F10),
            Ok(Operation::SkipIfRegistersNotEquals(0xF, 0x1))
        );
    }

    // ---------- ANNN ----------
    #[test]
    fn set_i() {
        assert_eq!(decode(0xAABC), Ok(Operation::SetI(0x0ABC)));
        assert_eq!(decode(0xA000), Ok(Operation::SetI(0x0000)));
        assert_eq!(decode(0xAFFF), Ok(Operation::SetI(0x0FFF)));
    }

    // ---------- BNNN ----------
    #[test]
    fn goto_plus_v0() {
        assert_eq!(decode(0xBABC), Ok(Operation::GotoPlusV0(0x0ABC)));
        assert_eq!(decode(0xB000), Ok(Operation::GotoPlusV0(0x0000)));
        assert_eq!(decode(0xBFFF), Ok(Operation::GotoPlusV0(0x0FFF)));
    }

    // ---------- CXNN ----------
    #[test]
    fn random() {
        assert_eq!(decode(0xCA12), Ok(Operation::Random(0xA, 0x12)));
        assert_eq!(decode(0xC000), Ok(Operation::Random(0x0, 0x00)));
        assert_eq!(decode(0xCFFF), Ok(Operation::Random(0xF, 0xFF)));
    }

    // ---------- DXYN ----------
    #[test]
    fn draw() {
        assert_eq!(decode(0xDAB5), Ok(Operation::Draw(0xA, 0xB, 0x5)));
        assert_eq!(decode(0xD000), Ok(Operation::Draw(0x0, 0x0, 0x0)));
        assert_eq!(decode(0xDFFF), Ok(Operation::Draw(0xF, 0xF, 0xF)));
    }

    // ---------- EX9E ----------
    #[test]
    fn skip_if_key_equals() {
        assert_eq!(decode(0xE59E), Ok(Operation::SkipIfKeyEquals(0x5)));
        assert_eq!(decode(0xE09E), Ok(Operation::SkipIfKeyEquals(0x0)));
        assert_eq!(decode(0xEF9E), Ok(Operation::SkipIfKeyEquals(0xF)));
    }

    // ---------- EXA1 ----------
    #[test]
    fn skip_if_key_not_equals() {
        assert_eq!(decode(0xE5A1), Ok(Operation::SkipIfKeyNotEquals(0x5)));
        assert_eq!(decode(0xE0A1), Ok(Operation::SkipIfKeyNotEquals(0x0)));
        assert_eq!(decode(0xEFA1), Ok(Operation::SkipIfKeyNotEquals(0xF)));
    }

    // ---------- FX07 ----------
    #[test]
    fn set_to_delay() {
        assert_eq!(decode(0xF507), Ok(Operation::SetToDelay(0x5)));
        assert_eq!(decode(0xF007), Ok(Operation::SetToDelay(0x0)));
        assert_eq!(decode(0xFF07), Ok(Operation::SetToDelay(0xF)));
    }

    // ---------- FX0A ----------
    #[test]
    fn set_to_key() {
        assert_eq!(decode(0xF50A), Ok(Operation::SetToKey(0x5)));
        assert_eq!(decode(0xF00A), Ok(Operation::SetToKey(0x0)));
        assert_eq!(decode(0xFF0A), Ok(Operation::SetToKey(0xF)));
    }

    // ---------- FX15 ----------
    #[test]
    fn set_delay_timer() {
        assert_eq!(decode(0xF515), Ok(Operation::SetDelayTimer(0x5)));
        assert_eq!(decode(0xF015), Ok(Operation::SetDelayTimer(0x0)));
        assert_eq!(decode(0xFF15), Ok(Operation::SetDelayTimer(0xF)));
    }

    // ---------- FX18 ----------
    #[test]
    fn set_sound_timer() {
        assert_eq!(decode(0xF518), Ok(Operation::SetSoundTimer(0x5)));
        assert_eq!(decode(0xF018), Ok(Operation::SetSoundTimer(0x0)));
        assert_eq!(decode(0xFF18), Ok(Operation::SetSoundTimer(0xF)));
    }

    // ---------- FX1E ----------
    #[test]
    fn add_to_i() {
        assert_eq!(decode(0xF51E), Ok(Operation::AddToI(0x5)));
        assert_eq!(decode(0xF01E), Ok(Operation::AddToI(0x0)));
        assert_eq!(decode(0xFF1E), Ok(Operation::AddToI(0xF)));
    }

    // ---------- FX29 ----------
    #[test]
    fn set_i_to_sprite() {
        assert_eq!(decode(0xF529), Ok(Operation::SetIToSprite(0x5)));
        assert_eq!(decode(0xF029), Ok(Operation::SetIToSprite(0x0)));
        assert_eq!(decode(0xFF29), Ok(Operation::SetIToSprite(0xF)));
    }

    // ---------- FX33 ----------
    #[test]
    fn set_binary_coded_decimal() {
        assert_eq!(decode(0xF533), Ok(Operation::SetBinaryCodedDecimal(0x5)));
        assert_eq!(decode(0xF033), Ok(Operation::SetBinaryCodedDecimal(0x0)));
        assert_eq!(decode(0xFF33), Ok(Operation::SetBinaryCodedDecimal(0xF)));
    }

    // ---------- FX55 ----------
    #[test]
    fn register_dump() {
        assert_eq!(decode(0xF555), Ok(Operation::RegisterDump(0x5)));
        assert_eq!(decode(0xF055), Ok(Operation::RegisterDump(0x0)));
        assert_eq!(decode(0xFF55), Ok(Operation::RegisterDump(0xF)));
    }

    // ---------- FX65 ----------
    #[test]
    fn register_load() {
        assert_eq!(decode(0xF565), Ok(Operation::RegisterLoad(0x5)));
        assert_eq!(decode(0xF065), Ok(Operation::RegisterLoad(0x0)));
        assert_eq!(decode(0xFF65), Ok(Operation::RegisterLoad(0xF)));
    }

    // ---------- Invalid / reserved opcodes ----------
    #[test]
    fn invalid_opcodes_return_error() {
        // 5XY1..5XYF are invalid (only 5XY0 valid)
        assert!(decode(0x5AB1).is_err());
        assert!(decode(0x5ABF).is_err());
        // 9XY1..9XYF invalid
        assert!(decode(0x9AB1).is_err());
        // 8XY8..8XYD, 8XYF invalid
        assert!(decode(0x8AB8).is_err());
        assert!(decode(0x8ABF).is_err());
        // 0NNN is handled by catch-all; make sure 0x0xxx with unknown low byte is treated as CallAssemblySubroutine,
        // but 0x00E0 / 0x00EE are special. Test a raw 0x0123:
        assert_eq!(
            decode(0x0123),
            Ok(Operation::CallAssemblySubroutine(0x0123))
        );
        // EX with invalid low byte (not 0x9E / 0xA1)
        assert!(decode(0xE500).is_err());
        // FX with invalid low byte
        assert!(decode(0xF500).is_err());
    }
}
