use std::io::{self, Write};
use std::mem::size_of;

use bytemuck::bytes_of;

use crate::internal::align_offset;
use crate::std430::{AsStd430, Std430, WriteStd430};

/**
Type that enables writing correctly aligned `std430` values to a buffer.

`Writer` is useful when many values need to be laid out in a row that cannot be
represented by a struct alone, like dynamically sized arrays or dynamically
laid-out values.

## Example
In this example, we'll write a length-prefixed list of lights to a buffer.
`std430::Writer` helps align correctly, even across multiple structs, which can
be tricky and error-prone otherwise.

```glsl
struct PointLight {
    vec3 position;
    vec3 color;
    float brightness;
};

buffer POINT_LIGHTS {
    uint len;
    PointLight[] lights;
} point_lights;
```

```
use crevice::std430::{self, AsStd430};

#[derive(AsStd430)]
struct PointLight {
    position: mint::Vector3<f32>,
    color: mint::Vector3<f32>,
    brightness: f32,
}

let lights = vec![
    PointLight {
        position: [0.0, 1.0, 0.0].into(),
        color: [1.0, 0.0, 0.0].into(),
        brightness: 0.6,
    },
    PointLight {
        position: [0.0, 4.0, 3.0].into(),
        color: [1.0, 1.0, 1.0].into(),
        brightness: 1.0,
    },
];

# fn map_gpu_buffer_for_write() -> &'static mut [u8] {
#     Box::leak(vec![0; 1024].into_boxed_slice())
# }
let target_buffer = map_gpu_buffer_for_write();
let mut writer = std430::Writer::new(target_buffer);

let light_count = lights.len() as u32;
writer.write(&light_count)?;

// Crevice will automatically insert the required padding to align the
// PointLight structure correctly. In this case, there will be 12 bytes of
// padding between the length field and the light list.

writer.write(lights.as_slice())?;

# fn unmap_gpu_buffer() {}
unmap_gpu_buffer();

# Ok::<(), std::io::Error>(())
```
*/
pub struct Writer<'a, W> {
    writer: &'a mut W,
    offset: usize,
}

impl<'a, W: Write> Writer<'a, W> {
    /// Create a new `Writer`, wrapping a buffer, file, or other type that
    /// implements [`std::io::Write`].
    pub fn new(writer: &'a mut W) -> Self {
        Self { writer, offset: 0 }
    }

    /// Write a new value to the underlying buffer, writing zeroed padding where
    /// necessary.
    ///
    /// Returns the offset into the buffer that the value was written to.
    pub fn write<T>(&mut self, value: &T) -> io::Result<usize>
    where
        T: WriteStd430 + ?Sized,
    {
        value.write_std430(self)
    }

    /// Write an iterator of values to the underlying buffer.
    ///
    /// Returns the offset into the buffer that the first value was written to.
    /// If no values were written, returns the `len()`.
    pub fn write_iter<I, T>(&mut self, iter: I) -> io::Result<usize>
    where
        I: IntoIterator<Item = T>,
        T: WriteStd430,
    {
        let mut offset = self.offset;

        let mut iter = iter.into_iter();

        if let Some(item) = iter.next() {
            offset = item.write_std430(self)?;
        }

        for item in iter {
            item.write_std430(self)?;
        }

        Ok(offset)
    }

    /// Write an `Std430` type to the underlying buffer.
    pub fn write_std430<T>(&mut self, value: &T) -> io::Result<usize>
    where
        T: Std430,
    {
        let padding = align_offset(self.offset, T::ALIGNMENT);

        for _ in 0..padding {
            self.writer.write_all(&[0])?;
        }
        self.offset += padding;

        let value = value.as_std430();
        self.writer.write_all(bytes_of(&value))?;

        let write_here = self.offset;
        self.offset += size_of::<T>();

        Ok(write_here)
    }

    /// Returns the amount of data written by this `Writer`.
    pub fn len(&self) -> usize {
        self.offset
    }
}
