mod stream;

use cpal::traits::StreamTrait;
use ringbuf::HeapRb;
use ringbuf::traits::Split;
use crate::stream::{InputStream, OutputStream};

fn main() -> Result<(), anyhow::Error> {
    let ring_buffer = HeapRb::<f32>::new(2048);
    let (producer, consumer) = ring_buffer.split();

    let input_stream = InputStream::open(producer);
    let output_stream = OutputStream::open(consumer);

    input_stream.stream.play()?;
    output_stream.stream.play()?;

    loop {}
}
