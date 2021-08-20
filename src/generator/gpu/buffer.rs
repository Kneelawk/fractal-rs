//! buffer.rs - This is a general purpose WGSL buffer wrapper that is copied
//! between my different WGSL projects.

// Because this file is coped often, not all projects use all the methods supplied here.
#![allow(dead_code)]

use bytemuck::{cast_slice, Pod};
use std::{marker::PhantomData, mem::size_of};
use wgpu::{
    Buffer, BufferAddress, BufferAsyncError, BufferDescriptor, BufferUsages, CommandBuffer,
    CommandEncoderDescriptor, Device, MapMode,
};

/// Statically-sized wrapper around a GPU buffer.
pub struct BufferWrapper<D: Encodable + Sized> {
    buffer: Buffer,
    staging_buffer: Option<Buffer>,
    capacity: BufferAddress,
    staging_capacity: BufferAddress,
    size: BufferAddress,

    _marker: PhantomData<D>,
}

impl<D: Encodable + Sized> BufferWrapper<D> {
    /// The size in bytes of this buffer's data type.
    pub fn data_size() -> BufferAddress {
        D::size() as BufferAddress
    }

    /// Creates a new buffer wrapper with the given data and usage.
    pub fn from_data(
        device: &Device,
        data: &[D],
        usage: BufferUsages,
    ) -> (BufferWrapper<D>, CommandBuffer) {
        let size = data.len() as BufferAddress;
        let buffer_size = size * BufferWrapper::<D>::data_size();
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("wrapped_staging_buffer"),
            size: buffer_size,
            usage: BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        D::encode_slice(
            data,
            staging_buffer.slice(..).get_mapped_range_mut().as_mut(),
        );
        staging_buffer.unmap();

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("wrapped_buffer"),
            size: buffer_size,
            usage: usage | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("initial_buffer_staging_encoder"),
        });

        encoder.copy_buffer_to_buffer(&staging_buffer, 0, &buffer, 0, buffer_size);

        (
            BufferWrapper {
                buffer,
                staging_buffer: Some(staging_buffer),
                capacity: size,
                staging_capacity: size,
                size,
                _marker: PhantomData,
            },
            encoder.finish(),
        )
    }

    /// Creates a new buffer wrapper with the given capacity.
    pub fn new(device: &Device, capacity: BufferAddress, usage: BufferUsages) -> BufferWrapper<D> {
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("wrapped_buffer"),
            size: capacity * BufferWrapper::<D>::data_size(),
            usage: usage | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        BufferWrapper {
            buffer,
            staging_buffer: None,
            capacity,
            staging_capacity: 0,
            size: 0,
            _marker: PhantomData,
        }
    }

    /// Gets this buffer's size.
    pub fn len(&self) -> BufferAddress {
        self.size
    }

    /// Gets this BufferWrapper's wrapped buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Effectively clears the data from this buffer.
    pub fn clear(&mut self) {
        self.size = 0;
    }

    /// Removes a number of instances from the end of this buffer.
    pub fn remove_last(&mut self, instances: BufferAddress) -> Result<(), BufferRemoveError> {
        if self.size >= instances {
            self.size -= instances;
            Ok(())
        } else {
            Err(BufferRemoveError::InsufficientSize)
        }
    }

    /// Sets the contents of this buffer.
    pub async fn replace_all(
        &mut self,
        device: &Device,
        data: &[D],
    ) -> Result<CommandBuffer, BufferWriteError> {
        let data_len = data.len() as BufferAddress;

        if data_len > self.capacity {
            return Err(BufferWriteError::InsufficientCapacity);
        }

        self.ensure_staging_capacity(device, data_len);

        let staging_buffer = self.staging_buffer.as_ref().unwrap();

        {
            let staging_slice = staging_buffer.slice(..);
            staging_slice.map_async(MapMode::Write).await?;
            let mut mapping = staging_slice.get_mapped_range_mut();
            D::encode_slice(data, mapping.as_mut());
        }

        staging_buffer.unmap();

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("buffer_staging_encoder"),
        });

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &self.buffer,
            0,
            data_len * BufferWrapper::<D>::data_size(),
        );

        // TODO: should we update size here?
        // Even though the command to copy the data to the actual buffer has not been
        // submitted yet?
        self.size = data_len;

        Ok(encoder.finish())
    }

    /// Append data to the end of this buffer if there is space remaining.
    pub async fn append(
        &mut self,
        device: &Device,
        data: &[D],
    ) -> Result<CommandBuffer, BufferWriteError> {
        let data_len = data.len() as BufferAddress;

        if self.size + data_len > self.capacity {
            return Err(BufferWriteError::InsufficientCapacity);
        }

        self.ensure_staging_capacity(device, data_len);

        let staging_buffer = self.staging_buffer.as_ref().unwrap();

        {
            let staging_slice = staging_buffer.slice(..);
            staging_slice.map_async(MapMode::Write).await?;
            let mut mapping = staging_slice.get_mapped_range_mut();
            D::encode_slice(data, mapping.as_mut());
        }

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("buffer_staging_encoder"),
        });

        encoder.copy_buffer_to_buffer(
            staging_buffer,
            0,
            &self.buffer,
            self.size * BufferWrapper::<D>::data_size(),
            data_len * BufferWrapper::<D>::data_size(),
        );

        // TODO: address potential issues with setting size here.
        self.size += data_len;

        Ok(encoder.finish())
    }

    /// Makes sure there is enough space in the staging buffer to handle
    /// whatever needs the staging buffer.
    fn ensure_staging_capacity(&mut self, device: &Device, size: BufferAddress) {
        if self.staging_buffer.is_none() || self.staging_capacity < size {
            self.staging_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("wrapped_staging_buffer"),
                size: size * BufferWrapper::<D>::data_size(),
                usage: BufferUsages::MAP_WRITE | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));
            self.staging_capacity = size;
        }
    }
}

/// Error potentially returned from write operations.
#[derive(Debug, Copy, Clone)]
pub enum BufferWriteError {
    InsufficientCapacity,
    BufferAsyncError,
}

impl From<BufferAsyncError> for BufferWriteError {
    fn from(_: BufferAsyncError) -> Self {
        BufferWriteError::BufferAsyncError
    }
}

/// Error potentially returned from remove operations.
#[derive(Debug, Copy, Clone)]
pub enum BufferRemoveError {
    InsufficientSize,
}

/// Trait used to help encode objects to buffers.
pub trait Encodable: Sized {
    /// Gets the size of this encodable.
    fn size() -> usize;

    /// Encodes a whole slice.
    fn encode_slice(slice: &[Self], write_to: &mut [u8]) {
        for (index, s) in slice.iter().enumerate() {
            s.encode(&mut write_to[(index * Self::size())..((index + 1) * Self::size())]);
        }
    }

    /// Encodes a single element.
    fn encode(&self, write_to: &mut [u8]);
}

impl<E: Pod + Clone> Encodable for E {
    fn size() -> usize {
        size_of::<Self>()
    }

    fn encode_slice(slice: &[Self], write_to: &mut [u8]) {
        write_to.copy_from_slice(cast_slice(slice));
    }

    fn encode(&self, write_to: &mut [u8]) {
        write_to.copy_from_slice(cast_slice(&[self.clone()]));
    }
}
