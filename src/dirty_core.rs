use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Host, SampleRate, SizedSample, Stream, StreamConfig,
};

use crate::{dirty_ui, BUFFER_SIZE};

pub struct AudioSys {
    _host: Host,
    input_device: Device,
    output_device: Device,

    pub config: StreamConfig,

    pub channels: Vec<Arc<Mutex<Channel>>>,
}

impl AudioSys {
    pub fn new() -> Result<Self> {
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
            channels: vec![Arc::new(Mutex::new(Channel::new()))],
        })
    }

    pub fn run(&mut self, ui_rx: Receiver<dirty_ui::UIMessage>) -> Result<()> {
        let mut buffer = Buffer::<f32>::new(BUFFER_SIZE * self.config.channels as usize);
        let mut listener = buffer.listen();

        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            buffer.write(Vec::from(data));
        };

        let volume = Arc::clone(self.channels.first().unwrap());

        let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let input = listener.read();
            let input: Vec<f32> = input.iter().map(|&s| s * volume.lock().unwrap().volume).collect();
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

#[derive(Clone)]
pub struct Channel {
    pub volume: f32,
}

impl Channel {
    pub fn new() -> Self {
        Self { volume: 1. }
    }
}

impl Default for Channel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct Buffer<T> {
    buffer: Arc<Mutex<Vec<T>>>,
}

pub struct BufferListener<T> {
    buffer: Arc<Mutex<Vec<T>>>,
}

impl<T: Clone + Default> Buffer<T> {
    pub fn new(buffer_size: usize) -> Buffer<T> {
        Buffer::<T> {
            buffer: Arc::new(Mutex::new(vec![T::default(); buffer_size])),
        }
    }

    pub fn write(&mut self, data: Vec<T>) {
        self.buffer.lock().unwrap().splice(.., data);
    }

    pub fn listen(&self) -> BufferListener<T> {
        BufferListener {
            buffer: Arc::clone(&self.buffer),
        }
    }
}

impl<T: Clone> BufferListener<T> {
    pub fn read(&mut self) -> Vec<T> {
        self.buffer.lock().unwrap().to_vec()
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
