use anyhow::{Ok, Result};
use dirty_core::AudioSys;
use dirty_ui::DirtyUI;
use eframe::{run_native, NativeOptions};
use tokio::task::spawn_blocking;

pub mod dirty_core;
pub mod dirty_ui;

const BUFFER_SIZE: usize = 16;

#[tokio::main]
async fn main() -> Result<()> {
    // initiate the audio
    let mut audio_sys = AudioSys::new()?;

    // initiate ui
    let (ui, ui_rx) = DirtyUI::new(&audio_sys);

    // launch threads
    let audio_handle = spawn_blocking(move || audio_sys.run(ui_rx));
    let _ = run_native(
        "dirty",
        NativeOptions::default(),
        Box::new(|_cc| std::result::Result::Ok(Box::<DirtyUI>::new(ui))),
    );

    // return
    audio_handle.await??;

    Ok(())
}
