use std::sync::{Arc, Mutex};

use anyhow::{Context, Ok, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Host, SampleRate, StreamConfig,
};
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};

use crate::{dirty_ui, BUFFER_SIZE};

use super::{
    audio_system::{InputSystem, OutputSystem, OutputSystemMessage},
    channel::{Channel, ChannelMessage},
    BuffVec,
};

pub type Float = f64;

pub struct DirtyCore {
    _host: Host,
    input_device: Device,
    output_device: Device,

    pub input_config: StreamConfig,
    pub output_config: StreamConfig,

    pub channels: Arc<Mutex<Vec<Channel>>>,

    _core_rx: Receiver<DirtyCoreMessage>,
    core_tx: Sender<DirtyCoreMessage>,
}

impl DirtyCore {
    pub fn new_default() -> Result<Self> {
        let host = cpal::default_host();
        let input_device = host
            .default_input_device()
            .context("no default input device")?;
        let output_device = host
            .default_output_device()
            .context("no default output device")?;
        let mut input_config: StreamConfig = input_device.default_input_config()?.into();
        let mut output_config: StreamConfig = input_device.default_output_config()?.into();
        input_config.sample_rate = SampleRate(48000);
        output_config.sample_rate = SampleRate(48000);
        input_config.buffer_size = BufferSize::Fixed(BUFFER_SIZE as u32);
        output_config.buffer_size = BufferSize::Fixed(BUFFER_SIZE as u32);

        let (core_tx, core_rx) = channel::<DirtyCoreMessage>(1024);

        Ok(Self {
            _host: host,
            input_device,
            output_device,
            input_config,
            output_config,
            channels: Arc::new(Mutex::new(vec![Channel::new(core_tx.clone())])),
            _core_rx: core_rx,
            core_tx,
        })
    }

    pub fn get_tx(&self) -> Sender<DirtyCoreMessage> {
        self.core_tx.clone()
    }

    pub async fn run(&self, mut ui_rx: Receiver<dirty_ui::UIMessage>) -> Result<()> {
        let num_channels = self.input_config.channels as usize;
        let output_buffers: Arc<Mutex<BuffVec<Float>>> =
            Arc::new(Mutex::new(BuffVec::new(num_channels)));
        let input_data_fn = move |data: &[Float], _: &cpal::InputCallbackInfo| {
            let _input_buffers = Arc::new(BuffVec::deinterlace(data, num_channels));
        };

        let channels = Arc::clone(&self.channels);
        let output_data_fn = move |data: &mut [Float], _: &cpal::OutputCallbackInfo| {
            let input: Vec<Float> = output_buffers.lock().unwrap().clone().collect();
            let binding = channels.lock().expect("lock failed");
            let channel = binding.first().expect("no channels");
            let input: Vec<Float> = input.iter().map(|s| s * channel.volume).collect();
            data.copy_from_slice(&input[..data.len()]);
        };

        let input_stream = InputSystem::get_input_stream(
            &self.input_device,
            &self.input_config,
            input_data_fn,
            err_fn,
            None,
        )?;
        let output_stream = OutputSystem::get_output_stream(
            &self.output_device,
            &self.output_config,
            output_data_fn,
            err_fn,
            None,
        )?;

        input_stream.play()?;
        output_stream.play()?;

        ui_rx.recv().await.context("ui tx dropped")?;
        Ok(())
    }
}

pub enum DirtyCoreMessage {
    GetOutputSystem(oneshot::Sender<Sender<OutputSystemMessage>>),
    GetChannel(usize, oneshot::Sender<Result<Sender<ChannelMessage>>>),

    NewChannel,

    NewBuffer,
}

#[derive(Clone, Copy)]
pub enum AudioIO {
    None,
    Hardware(PhysicalAudioIO),
}

#[derive(Clone, Copy)]
pub enum PhysicalAudioIO {
    Mono(usize),
    Stereo(usize, usize),
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
