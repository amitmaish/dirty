use anyhow::{Ok, Result};
use dirty_core::core::DirtyCore;
use dirty_ui::DirtyUI;
use eframe::{run_native, NativeOptions};

pub mod dirty_core;
pub mod dirty_ui;

const BUFFER_SIZE: usize = 16;

#[tokio::main]
async fn main() -> Result<()> {
    let core_sys = DirtyCore::new_default()?;

    let (ui, ui_rx) = DirtyUI::new(&core_sys);

    let core_future = core_sys.run(ui_rx);

    let _ = run_native(
        "dirty",
        NativeOptions::default(),
        Box::new(|_cc| std::result::Result::Ok(Box::<DirtyUI>::new(ui))),
    );

    core_future.await?;

    Ok(())
}
