use anyhow::{Ok, Result};
use cpal::traits::StreamTrait;
use dirty_core::{AudioSys, Buffer};
use dirty_ui::DirtyUI;
use eframe::{run_native, NativeOptions};

pub mod dirty_core;
pub mod dirty_ui;

const BUFFER_SIZE: usize = 16;

#[tokio::main]
async fn main() -> Result<()> {

    // initiate ui

    let ui = DirtyUI::default();

    let _ = run_native(
        "dirty",
        NativeOptions::default(),
        Box::new(|_cc| std::result::Result::Ok(Box::<DirtyUI>::new(ui))),
    );

    // initiate the audio
    let audio_sys = AudioSys::new_default()?;

    let mut buffer = Buffer::<f32>::new(BUFFER_SIZE * audio_sys.config.channels as usize);
    let mut listener = buffer.listen();

    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        buffer.write(Vec::from(data));
    };

    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
        let input = listener.read();
        //let input: Vec<f32> = input.iter().map(|&s| s * 1.0).collect();
        data.copy_from_slice(&input[..data.len()]);
    };

    let input_stream = audio_sys.get_default_input_stream(input_data_fn, err_fn, None);
    let output_stream = audio_sys.get_default_output_stream(output_data_fn, err_fn, None);

    input_stream.play()?;
    output_stream.play()?;
    // return
    Ok(())
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
