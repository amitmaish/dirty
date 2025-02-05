use eframe::{
    egui::{CentralPanel, Slider},
    App,
};

#[derive(Default)]
pub struct DirtyUI {
    pub volume: f32,
}

impl App for DirtyUI {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ui.add(
                Slider::new(&mut self.volume, 0.0..=1.0)
                    .vertical()
                    .text("volume"),
            );
        });
    }
}
