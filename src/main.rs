use chip_8_emu::*;

fn main() {
    let mut state = State::default();
    let bytes_read = state.load_game("./PONG");
    println!("{:?}", state);
    println!("Bytes read: {:?}", bytes_read);
}
