use std::sync::Arc;

use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{channel, Receiver, Sender},
        oneshot::{self},
    },
    task::spawn_blocking,
};

use super::{
    core::{AudioIO, DirtyCoreMessage, PhysicalAudioIO},
    output_system::OutputSystemMessage,
    BuffVec,
};

pub enum ChannelMessage {
    Quit,

    GetName(oneshot::Sender<String>),
    SetName(String),

    GetVolume(oneshot::Sender<f32>),
    SetVolume(f32),

    GetPanning(oneshot::Sender<f32>),
    SetPanning(f32),

    GetInput(oneshot::Sender<AudioIO>),
    SetInput(AudioIO),

    GetOutput(oneshot::Sender<AudioIO>),
    SetOutput(AudioIO),

    SetOuptutSystem(Sender<OutputSystemMessage>),

    RegisterMaster(Sender<()>),

    NewBuffer(Arc<BuffVec<f32>>),
}

pub struct Channel {
    channel_rx: Receiver<ChannelMessage>,
    channel_tx: Sender<ChannelMessage>,

    name: String,

    pub volume: f32,
    pub panning: f32,

    input: AudioIO,
    output: AudioIO,

    audio_system: Sender<DirtyCoreMessage>,
    output_system: Option<Sender<OutputSystemMessage>>,
}

impl Channel {
    pub fn new(audio_system: Sender<DirtyCoreMessage>) -> Self {
        let (channel_tx, channel_rx) = channel(16);
        Self {
            channel_rx,
            channel_tx,

            name: "".to_string(),

            volume: 1.,
            panning: 0.,

            input: AudioIO::None,
            output: AudioIO::None,

            audio_system,
            output_system: None,
        }
    }

    pub fn get_channel_tx(&self) -> Sender<ChannelMessage> {
        self.channel_tx.clone()
    }

    pub async fn run_channel(mut self) {
        loop {
            let message = self.channel_rx.recv().await.unwrap();
            match message {
                ChannelMessage::Quit => break,
                ChannelMessage::GetName(sender) => {
                    let _ = sender.send(self.name.clone());
                }
                ChannelMessage::SetName(name) => {
                    self.name = name;
                }
                ChannelMessage::GetVolume(sender) => {
                    let _ = sender.send(self.volume);
                }
                ChannelMessage::SetVolume(volume) => {
                    self.volume = volume;
                }
                ChannelMessage::GetPanning(sender) => {
                    let _ = sender.send(self.panning);
                }
                ChannelMessage::SetPanning(panning) => {
                    self.panning = panning;
                }
                ChannelMessage::GetInput(sender) => {
                    let _ = sender.send(self.input);
                }
                ChannelMessage::SetInput(input) => {
                    self.input = input;
                }
                ChannelMessage::GetOutput(sender) => {
                    let _ = sender.send(self.output);
                }
                ChannelMessage::SetOutput(output) => {
                    self.output = output;
                }

                ChannelMessage::SetOuptutSystem(sender) => {
                    self.output_system = Some(sender);
                }

                ChannelMessage::RegisterMaster(_sender) => {
                    todo!();
                }

                ChannelMessage::NewBuffer(data) => {
                    self.process_audio(data).await;
                }
            }
        }
    }

    async fn process_audio(&self, data: Arc<BuffVec<f32>>) {
        let input = self.input;
        let channel_volume = self.volume;

        let audio_system = self.audio_system.clone();
        let output_system = self.output_system.clone();
        let self_address = self.get_channel_tx();
        let _ = spawn_blocking(move || {
            let mut input_data = match input {
                AudioIO::None => None,
                AudioIO::Hardware(physical_audio_io) => match physical_audio_io {
                    PhysicalAudioIO::Mono(c) => Some(data.get_buffer(c).unwrap()),
                    PhysicalAudioIO::Stereo(l, _r) => Some(data.get_buffer(l).unwrap()), // Some(Buffer::<StereoSample>::from_vectors(
                                                                                         //     data.get_buffer(l).unwrap(),
                                                                                         //     data.get_buffer(_r).unwrap(),
                                                                                         // )),
                },
            }
            .unwrap();
            input_data.iter_mut().for_each(|s| {
                *s *= channel_volume;
            });

            Handle::current().block_on(async {
                match output_system {
                    None => (),
                    Some(tx) => {
                        if (tx.send(OutputSystemMessage::Overdub(input_data)).await).is_err() {
                            let (sender, reciever) = oneshot::channel();
                            audio_system
                                .send(DirtyCoreMessage::GetOutputSystem(sender))
                                .await;
                            let new_output_system = reciever.await.unwrap();
                            self_address
                                .send(ChannelMessage::SetOuptutSystem(new_output_system))
                                .await;
                        }
                    }
                }
            })
        })
        .await;
    }
}
