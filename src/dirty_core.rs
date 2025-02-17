use core::f32;
use std::{
    ops::AddAssign,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{Context, Ok, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Host, SampleRate, SizedSample, Stream, StreamConfig,
};
use tokio::sync::oneshot;

use crate::{dirty_ui, BUFFER_SIZE};

pub struct AudioSys {
    _host: Host,
    input_device: Device,
    output_device: Device,

    pub config: StreamConfig,

    pub channels: Arc<Mutex<Vec<Channel>>>,
}

impl AudioSys {
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

        Ok(Self {
            _host: host,
            input_device: input,
            output_device: output,
            config,
            channels: Arc::new(Mutex::new(vec![Channel::new()])),
        })
    }

    pub fn run(&mut self, ui_rx: Receiver<dirty_ui::UIMessage>) -> Result<()> {
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

        ui_rx.recv()?;
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

pub enum ChannelMessage {
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

    NewBuffer(Arc<BuffVec<f32>>),
}

pub struct Channel {
    _channel_rx: Receiver<ChannelMessage>,
    channel_tx: Sender<ChannelMessage>,

    _name: String,

    pub volume: f32,
    pub panning: f32,

    _input: AudioIO,
    _output: AudioIO,
}

impl Channel {
    pub fn new() -> Self {
        let (channel_tx, channel_rx) = channel();
        Self {
            _channel_rx: channel_rx,
            channel_tx,

            _name: "".to_string(),

            volume: 1.,
            panning: 0.,

            _input: AudioIO::None,
            _output: AudioIO::None,
        }
    }

    pub fn _get_channel_tx(&self) -> Sender<ChannelMessage> {
        self.channel_tx.clone()
    }

    fn _handle_message(&mut self, message: ChannelMessage) {
        match message {
            ChannelMessage::GetName(sender) => {
                let _ = sender.send(self._name.clone());
            }
            ChannelMessage::SetName(name) => {
                self._name = name;
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
                let _ = sender.send(self._input);
            }
            ChannelMessage::SetInput(input) => {
                self._input = input;
            }
            ChannelMessage::GetOutput(sender) => {
                let _ = sender.send(self._output);
            }
            ChannelMessage::SetOutput(output) => {
                self._output = output;
            }

            ChannelMessage::NewBuffer(data) => {
                self._process_audio(data);
            }
        }
    }

    fn _process_audio(&self, _data: Arc<BuffVec<f32>>) {}
}

impl Default for Channel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct Buffer<T> {
    buffer: Arc<Mutex<Vec<T>>>,
}

impl<T: AddAssign + Clone + Copy + Default + Sync> Buffer<T> {
    pub fn new(buffer_size: usize) -> Buffer<T> {
        Buffer::<T> {
            buffer: Arc::new(Mutex::new(vec![T::default(); buffer_size])),
        }
    }

    pub fn write(&mut self, data: Vec<T>) {
        self.buffer.lock().unwrap().splice(.., data);
    }

    pub fn _overdub(&mut self, data: Vec<T>) -> anyhow::Result<()> {
        let mut buffer = self.buffer.lock().unwrap();
        for (i, s) in buffer.iter_mut().enumerate() {
            *s += *data.get(i).context("mismatched buffer size")?;
        }
        Ok(())
    }

    pub fn _read(&mut self) -> Result<Vec<T>> {
        Ok(self.buffer.lock().unwrap().clone())
    }
}

#[derive(Clone)]
pub struct BuffVec<T> {
    data: Vec<Buffer<T>>,
    outer_pointer: usize,
    inner_pointer: usize,
}

impl<T: AddAssign + Clone + Copy + Default + Sync> BuffVec<T> {
    fn new(channels: usize) -> Self {
        Self {
            data: vec![Buffer::<T>::new(0); channels],
            outer_pointer: 0,
            inner_pointer: 0,
        }
    }

    fn get_next(&mut self) -> Option<T> {
        match self.data.get(self.outer_pointer) {
            None => {
                self.inner_pointer += 1;
                self.outer_pointer = 0;
                self.get_next()
            }
            Some(vec) => {
                //eprintln!("pointers: ({}, {})", self.outer_pointer, self.inner_pointer);
                self.outer_pointer += 1;
                vec.buffer.lock().unwrap().get(self.inner_pointer).cloned()
            }
        }
    }

    fn deinterlace(data: &[T], num_channels: usize) -> Self {
        let mut temp_buf = BuffVec::new(num_channels);
        temp_buf
            .data
            .iter_mut()
            .enumerate()
            .for_each(|(i, buffer)| {
                buffer.write(
                    data.split_at(i)
                        .1
                        .iter()
                        .step_by(num_channels)
                        .copied()
                        .collect(),
                );
            });
        temp_buf
    }
}

impl<T: AddAssign + Clone + Copy + Default + Sync> Iterator for BuffVec<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.get_next()
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
