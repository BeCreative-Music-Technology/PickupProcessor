use std::sync::Arc;
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use cpal::{FromSample, SizedSample};
use cpal::traits::{DeviceTrait, HostTrait};
use ringbuf::{CachingCons, CachingProd, SharedRb};
use ringbuf::storage::Heap;
use ringbuf::traits::{Consumer, Producer};

pub struct InputStream {
  pub stream: Stream,
}

impl InputStream {
  pub fn open(producer: CachingProd<Arc<SharedRb<Heap<f32>>>>) -> InputStream {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("Failed to get default input device");
    let config = device.default_input_config().expect("Failed to get default input config");

    let stream = match config.sample_format() {
      SampleFormat::I8 => Self::create_stream::<i8>(&device, &config.into(), producer),
      SampleFormat::I16 => Self::create_stream::<i16>(&device, &config.into(), producer),
      SampleFormat::I32 => Self::create_stream::<i32>(&device, &config.into(), producer),
      SampleFormat::I64 => Self::create_stream::<i64>(&device, &config.into(), producer),
      SampleFormat::U8 => Self::create_stream::<u8>(&device, &config.into(), producer),
      SampleFormat::U16 => Self::create_stream::<u16>(&device, &config.into(), producer),
      SampleFormat::U32 => Self::create_stream::<u32>(&device, &config.into(), producer),
      SampleFormat::U64 => Self::create_stream::<u64>(&device, &config.into(), producer),
      SampleFormat::F32 => Self::create_stream::<f32>(&device, &config.into(), producer),
      SampleFormat::F64 => Self::create_stream::<f64>(&device, &config.into(), producer),
      _ => panic!("Unsupported input format: {:?}", config.sample_format()),
    };

    InputStream { stream }
  }

  fn create_stream<T: SizedSample + FromSample<f64>>(
    device: &Device,
    config: &StreamConfig,
    mut producer: CachingProd<Arc<SharedRb<Heap<f32>>>>
  ) -> Stream where f32: FromSample<T> {
    device.build_input_stream(
      &config,
      move |data: &[T], _| {
        for &sample in data {
          let _ = producer.try_push(sample.to_sample::<f32>());
        }
      },
      |err| eprintln!("an error occurred on stream: {err}"),
      None,
    ).expect("Failed to create an input stream")
  }
}

pub struct OutputStream {
  pub stream: Stream,
}

impl OutputStream {
  pub fn open(consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>) -> OutputStream {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("Failed to get default output device");
    let config = device.default_output_config().expect("Failed to get default output config");

    let stream = match config.sample_format() {
      SampleFormat::I8 => Self::create_stream::<i8>(&device, &config.into(), consumer),
      SampleFormat::I16 => Self::create_stream::<i16>(&device, &config.into(), consumer),
      SampleFormat::I32 => Self::create_stream::<i32>(&device, &config.into(), consumer),
      SampleFormat::I64 => Self::create_stream::<i64>(&device, &config.into(), consumer),
      SampleFormat::U8 => Self::create_stream::<u8>(&device, &config.into(), consumer),
      SampleFormat::U16 => Self::create_stream::<u16>(&device, &config.into(), consumer),
      SampleFormat::U32 => Self::create_stream::<u32>(&device, &config.into(), consumer),
      SampleFormat::U64 => Self::create_stream::<u64>(&device, &config.into(), consumer),
      SampleFormat::F32 => Self::create_stream::<f32>(&device, &config.into(), consumer),
      SampleFormat::F64 => Self::create_stream::<f64>(&device, &config.into(), consumer),
      _ => panic!("Unsupported output format: {:?}", config.sample_format()),
    };

    OutputStream { stream }
  }

  fn create_stream<T>(
    device: &Device,
    config: &StreamConfig,
    mut consumer: CachingCons<Arc<SharedRb<Heap<f32>>>>
  ) -> Stream
  where T: SizedSample + FromSample<f64> {
    device.build_output_stream(
      &config,
      move |output: &mut [f32], _| {
        for sample in output.iter_mut() {
          *sample = consumer.try_pop().unwrap_or(0.0);
        }
      },
      |err| eprintln!("an error occurred on stream: {err}"),
      None,
    ).expect("Failed to create an input stream")
  }
}
