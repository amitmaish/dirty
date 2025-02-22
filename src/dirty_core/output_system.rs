use tokio::sync::mpsc::{Receiver, Sender};

pub enum OutputSystemMessage {
    Overdub(Vec<f32>),
}

pub struct OutputSystem {
    _output_rx: Receiver<OutputSystemMessage>,
    _output_tx: Sender<OutputSystemMessage>,
}
