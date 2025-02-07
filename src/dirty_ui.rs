use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use eframe::{
    egui::{CentralPanel, Context, Slider, Ui},
    App,
};

use crate::dirty_core::{AudioSys, Channel};

pub enum UIMessage {
    Quit,
}

pub struct DirtyUI {
    channels: Arc<Mutex<Vec<Channel>>>,

    ui_tx: Sender<UIMessage>,
}

impl DirtyUI {
    pub fn new(audio_sys: &AudioSys) -> (Self, Receiver<UIMessage>) {
        let (ui_tx, ui_rx) = mpsc::channel();
        (
            Self {
                channels: Arc::clone(&audio_sys.channels),
                ui_tx,
            },
            ui_rx,
        )
    }
}

pub trait FaderUI {
    fn draw_fader(&mut self, ui: &mut Ui);
}

impl FaderUI for Channel {
    fn draw_fader(&mut self, ui: &mut Ui) {
        let fader = ui.add(
            Slider::new(&mut self.volume, 0.0..=1.)
                .vertical()
                .text("volume"),
        );
        if fader.double_clicked() {
            self.volume = 1.0;
        }

        let panning = ui.add(Slider::new(&mut self.panning, -1.0..=1.0).text("pan"));
        if panning.double_clicked() {
            self.panning = 0.0;
        }
    }
}

impl App for DirtyUI {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            let mut channels = self.channels.lock().expect("channels lock failed");
            for channel in &mut *channels {
                channel.draw_fader(ui);
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.ui_tx.send(UIMessage::Quit);
    }
}
