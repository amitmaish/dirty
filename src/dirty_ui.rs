use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc, Mutex,
};

use eframe::{
    egui::{CentralPanel, Context, Slider, Ui},
    App,
};

use crate::dirty_core::Channel;

pub enum UIMessage {
    Quit,
    VolumeChanged(f32),
}

pub struct DirtyUI {
    channels: Vec<Arc<Mutex<Channel>>>,

    ui_tx: Sender<UIMessage>,
}

impl DirtyUI {
    pub fn new() -> (Self, Receiver<UIMessage>) {
        let (ui_tx, ui_rx) = mpsc::channel();
        (
            Self {
                channels: Vec::<Arc<Mutex<Channel>>>::new(),
                ui_tx,
            },
            ui_rx,
        )
    }

    pub fn register_channel(&mut self, channel: &Arc<Mutex<Channel>>) {
        self.channels.push(Arc::clone(channel));
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
            eprintln!("double clicked");
            self.volume = 1.0;
        }
    }
}

impl App for DirtyUI {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            for channel in &mut self.channels {
                channel.lock().unwrap().draw_fader(ui);
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.ui_tx.send(UIMessage::Quit);
    }
}
