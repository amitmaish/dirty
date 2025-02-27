use std::{sync::Arc, time::Duration};

use anyhow::Result;
use cpal::{traits::DeviceTrait, Device, SizedSample, Stream, StreamConfig};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use super::{
    core::{DirtyCoreMessage, Float},
    BuffVec,
};

pub struct InputSystem<'a> {
    stream: Option<Stream>,

    _input_rx: Receiver<InputSystemMessage<'a>>,
    input_tx: Sender<InputSystemMessage<'a>>,

    _core_tx: Sender<DirtyCoreMessage>,

    _output_tx: Option<Sender<OutputSystemMessage>>,
}

impl<'a> InputSystem<'a> {
    pub fn new(core_tx: Sender<DirtyCoreMessage>) -> Self {
        let (input_tx, input_rx) = channel(128);
        Self {
            stream: None,
            _input_rx: input_rx,
            input_tx,
            _core_tx: core_tx,
            _output_tx: None,
        }
    }

    pub fn start(self) -> Sender<InputSystemMessage<'a>> {
        self.input_tx
    }

    pub fn get_input_stream<T, D, E>(
        device: &Device,
        config: &StreamConfig,
        data_callback: D,
        error_callback: E,
        timeout: Option<Duration>,
    ) -> Result<Stream>
    where
        T: SizedSample,
        D: FnMut(&[T], &cpal::InputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        Ok(device.build_input_stream(config, data_callback, error_callback, timeout)?)
    }

    async fn _handle_messages(mut self) {
        loop {
            match self
                ._input_rx
                .recv()
                .await
                .unwrap_or(InputSystemMessage::Quit)
            {
                InputSystemMessage::NewInputSource(device, stream_config) => match self.stream {
                    Some(_stream) => todo!(),
                    None => {
                        self.stream = {
                            let num_channels = stream_config.channels as usize;
                            let stream = Self::get_input_stream(
                                device,
                                stream_config,
                                move |data: &[Float], callback: &cpal::InputCallbackInfo| {
                                    Self::input_data_function(data, callback, num_channels)
                                },
                                err_fn,
                                None,
                            );
                            match stream {
                                Ok(stream) => Some(stream),
                                Err(_) => None,
                            }
                        }
                    }
                },
                InputSystemMessage::Quit => break,
            }
        }
    }

    fn input_data_function(data: &[Float], _: &cpal::InputCallbackInfo, num_channels: usize) {
        let _input_buffers = Arc::new(BuffVec::deinterlace(data, num_channels));
    }
}

pub enum InputSystemMessage<'a> {
    NewInputSource(&'a Device, &'a StreamConfig),
    Quit,
}

pub struct OutputSystem {
    _output_rx: Receiver<OutputSystemMessage>,
    output_tx: Sender<OutputSystemMessage>,

    _core_tx: Sender<DirtyCoreMessage>,
}

impl OutputSystem {
    pub fn new(core_tx: Sender<DirtyCoreMessage>) -> Self {
        let (input_tx, input_rx) = channel(128);
        Self {
            _output_rx: input_rx,
            output_tx: input_tx,
            _core_tx: core_tx,
        }
    }

    pub fn start(self) -> Sender<OutputSystemMessage> {
        self.output_tx
    }

    pub fn get_output_stream<T, D, E>(
        device: &Device,
        config: &StreamConfig,
        data_callback: D,
        error_callback: E,
        timeout: Option<Duration>,
    ) -> Result<Stream>
    where
        T: SizedSample,
        D: FnMut(&mut [T], &cpal::OutputCallbackInfo) + Send + 'static,
        E: FnMut(cpal::StreamError) + Send + 'static,
    {
        Ok(device.build_output_stream(config, data_callback, error_callback, timeout)?)
    }
}

pub enum OutputSystemMessage {
    NewInput,
    Overdub(Vec<Float>),
    Quit,
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}
