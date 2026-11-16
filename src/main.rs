use std::time::{Duration, Instant};

use chip_8_emu::state::State;
use macroquad::{
    audio::load_sound,
    color::{BLACK, WHITE},
    input::is_key_down,
    math::vec2,
    texture::{DrawTextureParams, FilterMode, Image, Texture2D, draw_texture_ex},
    window::{Conf, clear_background, next_frame, screen_height, screen_width},
};

const OPERATIONS_PER_SECOND: u64 = 800;
const FRAMES_PER_SECOND: f64 = 60.0;
const BEEP_FILE: &str = "assets/beep.wav";

fn window_config() -> Conf {
    if std::env::args().len() != 2 {
        eprintln!("Usage:\n\nchip_8_emu [PROGRAM]");
        std::process::exit(1);
    }

    Conf {
        window_title: "CHIP 8 emulator".to_string(),
        window_width: 800,
        window_height: 400,
        ..Default::default()
    }
}

fn create_image(image: &mut Image, screen_data: &[[u8; 32]; 64]) {
    let image_data = image.get_image_data_mut();

    for y in 0..32 {
        for x in 0..64 {
            let color = if screen_data[x][y] == 1 { WHITE } else { BLACK };
            image_data[y * 64 + x] = color.into();
        }
    }
}

fn draw_on_screen(texture: &Texture2D) {
    clear_background(BLACK);
    draw_texture_ex(
        texture,
        0.0,
        0.0,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(screen_width(), screen_height())),
            ..Default::default()
        },
    );
}

async fn game_loop(state: State) {
    let mut state = state;
    let mut image = Image::gen_image_color(64, 32, WHITE);
    let texture = Texture2D::from_image(&image);
    texture.set_filter(FilterMode::Nearest);

    // Limit to 60 FPS
    let target_frame_time = Duration::from_secs_f64(1.0 / FRAMES_PER_SECOND);

    let mut op_budget: f64 = 0.0;
    let mut last_frame = Instant::now();

    loop {
        let frame_start = Instant::now();

        op_budget +=
            frame_start.duration_since(last_frame).as_secs_f64() * OPERATIONS_PER_SECOND as f64;
        last_frame = frame_start;

        while op_budget >= 1.0 {
            match state.get_next_operation() {
                Ok(op) => state.perform_operation(op),
                Err(e) => {
                    println!("{e}");
                    return;
                }
            }
            op_budget -= 1.0;
        }

        state.update_timers();
        state.beep();

        for (key, register) in &state.key_mappings {
            state.keys[*register] = is_key_down(*key);
        }

        create_image(&mut image, &state.screen_data);
        texture.update(&image);
        draw_on_screen(&texture);

        next_frame().await;

        // Cap FPS
        let elapsed = frame_start.elapsed();
        if elapsed < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed);
        }
    }
}

#[macroquad::main(window_config)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let game_file = &args[1];

    let beep_sound = match load_sound(BEEP_FILE).await {
        Ok(sound) => sound,
        Err(error) => {
            println!("Could not load beep file. {error}");
            return;
        }
    };

    let mut state = State::new(beep_sound);

    match state.load_game(game_file) {
        Ok(_bytes_read) => {
            game_loop(state).await;
        }
        Err(error) => println!("Error {error}"),
    }
}
