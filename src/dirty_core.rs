use core::f32;
use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Duration,
    vec,
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

    pub channels: Arc<Mutex<Vec<Channel>>>,
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
            channels: Arc::new(Mutex::new(vec![Channel::new()])),
        })
    }

    pub fn run(&mut self, ui_rx: Receiver<dirty_ui::UIMessage>) -> Result<()> {
        let mut buffers = Vec::<Buffer<f32>>::new();

        for _ in 0..self.config.channels {
            buffers.push(Buffer::<f32>::new(BUFFER_SIZE));
        }

        let listeners: BuffVec<f32> =
            BuffVec::<f32>::new(buffers.iter().map(|b| b.listen()).collect());

        let num_channels = self.config.channels as usize;
        let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            buffers.iter_mut().enumerate().for_each(|(i, buffer)| {
                buffer.write(
                    data.split_at(i)
                        .1
                        .iter()
                        .step_by(num_channels)
                        .copied()
                        .collect(),
                );
            });
        };

        let channels = Arc::clone(&self.channels);
        let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let input: Vec<f32> = listeners.clone().collect();
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
    Mono(usize),
    Stereo(usize, usize),
}

#[derive(Clone, Copy)]
pub struct Channel {
    pub volume: f32,
    pub panning: f32,

    pub _input: AudioIO,
    pub _output: AudioIO,
}

impl Channel {
    pub fn new() -> Self {
        Self {
            volume: 1.,
            panning: 0.,

            _input: AudioIO::Mono(0),
            _output: AudioIO::Stereo(0, 1),
        }
    }
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

#[derive(Clone)]
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

    #[cfg(test)]
    fn to_vec(&self) -> Vec<T> {
        let vec = self.buffer.lock().unwrap().to_vec();
        vec
    }
}

impl<T: Clone> BufferListener<T> {
    pub fn read(&mut self) -> Vec<T> {
        self.buffer.lock().unwrap().to_vec()
    }
}

#[derive(Clone)]
struct BuffVec<T> {
    data: Vec<BufferListener<T>>,
    outer_pointer: usize,
    inner_pointer: usize,
}

impl<T: Clone> BuffVec<T> {
    fn new(data: Vec<BufferListener<T>>) -> Self {
        Self {
            data,
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
}

impl<T: Clone> Iterator for BuffVec<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.get_next().clone()
    }
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn deinterlace_data() {
        let mut buffers = [
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
            Buffer::<usize>::new(BUFFER_SIZE),
        ];
        let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let num_channels = 10;

        buffers.iter_mut().enumerate().for_each(|(i, buffer)| {
            let data = data
                .split_at(i)
                .1
                .iter()
                .step_by(num_channels)
                .copied()
                .collect::<Vec<usize>>();
            eprintln!("{:?}", data);
            buffer.write(data);
        });

        let buffers: Vec<Vec<usize>> = buffers.iter().map(|v| v.clone().to_vec()).collect();

        assert_eq!(
            buffers,
            vec![
                vec![0, 0],
                vec![1, 1],
                vec![2, 2],
                vec![3, 3],
                vec![4, 4],
                vec![5, 5],
                vec![6, 6],
                vec![7, 7],
                vec![8, 8],
                vec![9, 9]
            ]
        )
    }

    #[test]
    fn interlace_data() {
        let data = BuffVec::new(vec![
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
            Buffer::<usize>::new(1).listen(),
        ]);

        assert_eq!(data.clone().collect::<Vec<usize>>(), vec![0; 10])
    }

    #[test]
    fn audio_integration() {
        let input_data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let num_channels = 10;

        let mut buffers = Vec::<Buffer<usize>>::new();

        for _ in 0..num_channels {
            buffers.push(Buffer::<usize>::new(BUFFER_SIZE));
        }

        let listeners: BuffVec<usize> =
            BuffVec::<usize>::new(buffers.iter().map(|b| b.listen()).collect());

        buffers.iter_mut().enumerate().for_each(|(i, buffer)| {
            let data = input_data
                .split_at(i)
                .1
                .iter()
                .step_by(num_channels)
                .copied()
                .collect::<Vec<usize>>();
            eprintln!("{:?}", data);
            buffer.write(data);
        });
        eprintln!("{:?}", buffers);

        let input: Vec<usize> = listeners.clone().collect();
        let mut output_data = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        output_data.copy_from_slice(&input[..input_data.len()]);

        assert_eq!(output_data, input_data)
    }
}
