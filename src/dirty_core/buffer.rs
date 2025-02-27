use std::ops::{Add, AddAssign, Mul, MulAssign};

use anyhow::{Context, Result};

use super::core::Float;

pub trait Sample: Add + AddAssign + Mul + MulAssign + Sized {}

impl Sample for Float {}

#[derive(Debug, Clone)]
pub struct Buffer<T> {
    data: Vec<T>,
}

impl<T: Clone + Copy + Default + Sample + Sync> Buffer<T> {
    pub fn new(buffer_size: usize) -> Buffer<T> {
        Buffer::<T> {
            data: vec![T::default(); buffer_size],
        }
    }

    pub fn write(&mut self, data: Vec<T>) {
        self.data.splice(.., data);
    }

    pub fn _overdub(&mut self, data: Vec<T>) -> anyhow::Result<()> {
        for (i, s) in self.data.iter_mut().enumerate() {
            *s += *data.get(i).context("mismatched buffer size")?;
        }
        Ok(())
    }

    pub fn read(&self) -> Result<Vec<T>> {
        Ok(self.data.clone())
    }
}

#[derive(Clone)]
pub struct BuffVec<T> {
    data: Vec<Buffer<T>>,
    outer_pointer: usize,
    inner_pointer: usize,
}

impl<T: Clone + Copy + Default + Sample + Sync> BuffVec<T> {
    pub fn new(channels: usize) -> Self {
        Self {
            data: vec![Buffer::<T>::new(0); channels],
            outer_pointer: 0,
            inner_pointer: 0,
        }
    }

    pub fn get_buffer(&self, index: usize) -> Result<Vec<T>> {
        self.data.get(index).context("out of bounds read")?.read()
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
                vec.data.get(self.inner_pointer).cloned()
            }
        }
    }

    pub fn deinterlace(data: &[T], num_channels: usize) -> Self {
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

impl<T: Clone + Copy + Default + Sample + Sync> Iterator for BuffVec<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.get_next()
    }
}
