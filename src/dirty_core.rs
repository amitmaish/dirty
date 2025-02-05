use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use cpal::{
    traits::{DeviceTrait, HostTrait},
    Device, Host, SizedSample, Stream, StreamConfig,
};

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

pub struct AudioSys {
    _host: Host,
    input_device: Device,
    output_device: Device,

    pub config: StreamConfig,
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
        let config = input.default_input_config()?.into();
        Ok(Self {
            _host: host,
            input_device: input,
            output_device: output,
            config,
        })
    }

    pub fn get_default_input_stream<T, D, E>(
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

    pub fn get_default_output_stream<T, D, E>(
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
