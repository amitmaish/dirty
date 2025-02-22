use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Host, SampleRate, SizedSample, Stream, StreamConfig,
};
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    oneshot,
};

use crate::{dirty_ui, BUFFER_SIZE};

use super::{output_system::OutputSystemMessage, BuffVec, Channel, ChannelMessage};

pub enum DirtyCoreMessage {
    GetOutputSystem(oneshot::Sender<Sender<OutputSystemMessage>>),
    GetChannel(usize, oneshot::Sender<Result<Sender<ChannelMessage>>>),

    NewBuffer,
}

pub struct DirtyCore {
    _host: Host,
    input_device: Device,
    output_device: Device,

    pub config: StreamConfig,

    pub channels: Arc<Mutex<Vec<Channel>>>,

    _audio_rx: Receiver<DirtyCoreMessage>,
    _audio_tx: Sender<DirtyCoreMessage>,
}

impl DirtyCore {
    pub fn new_default() -> Result<Self> {
        let host = cpal::default_host();
        let input = host
            .default_input_device()
            .context("no default input device")?;
        let output = host
            .default_output_device()
            .context("no default output device")?;
        let mut config: StreamConfig = input.default_input_config()?.into();
        config.sample_rate = SampleRate(48000);
        config.buffer_size = BufferSize::Fixed(BUFFER_SIZE as u32);

        let (audio_tx, audio_rx) = channel::<DirtyCoreMessage>(128);

        Ok(Self {
            _host: host,
            input_device: input,
            output_device: output,
            config,
            channels: Arc::new(Mutex::new(vec![Channel::new(audio_tx.clone())])),
            _audio_rx: audio_rx,
            _audio_tx: audio_tx,
        })
    }

    pub async fn run(&self, mut ui_rx: Receiver<dirty_ui::UIMessage>) -> Result<()> {
        let num_channels = self.config.channels as usize;
        let output_buffers: Arc<Mutex<BuffVec<f32>>> =
            Arc::new(Mutex::new(BuffVec::new(num_channels)));
        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let _input_buffers = Arc::new(BuffVec::deinterlace(data, num_channels));
        };

        let channels = Arc::clone(&self.channels);
        let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let input: Vec<f32> = output_buffers.lock().unwrap().clone().collect();
            let binding = channels.lock().expect("lock failed");
            let channel = binding.first().expect("no channels");
            let input: Vec<f32> = input.iter().map(|s| s * channel.volume).collect();
            data.copy_from_slice(&input[..data.len()]);
        };

        let input_stream = self.get_default_input_stream(input_data_fn, err_fn, None);
        let output_stream = self.get_default_output_stream(output_data_fn, err_fn, None);

        input_stream.play()?;
        output_stream.play()?;

        ui_rx.recv().await.context("ui tx dropped")?;
        Ok(())
    }

    // ----------------------------------------------------------

    fn get_default_input_stream<T, D, E>(
        &self,
        data_callback: D,
        error_callback: E,
        timeout: Option<Duration>,
    ) -> Stream
    where
        T: SizedSample,
        D: FnMut(&[T], &cpal::InputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        self.input_device
            .build_input_stream(&self.config, data_callback, error_callback, timeout)
            .expect("couldn't build default input stream")
    }

    fn get_default_output_stream<T, D, E>(
        &self,
        data_callback: D,
        error_callback: E,
        timeout: Option<Duration>,
    ) -> Stream
    where
        T: SizedSample,
        D: FnMut(&mut [T], &cpal::OutputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        self.output_device
            .build_output_stream(&self.config, data_callback, error_callback, timeout)
            .expect("couldn't build default output stream")
    }
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
