// ======================  THE SERVER SIDE OF THE REMOTE AUDIO TRANSMISSION ==================================

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;

use std::net::TcpListener;
use std::io::Write;

// Sample audio data type
type AudioSample = f32;

// Serialize function to convert audio samples to bytes
fn serialize_audio(samples: &AudioSample) -> Vec<u8> {
    bincode::serialize(samples).unwrap()
}

// Function to send audio data over TCP
fn send_audio_data(samples: &AudioSample) -> std::io::Result<()> {
    let listener = TcpListener::bind("localhost:1234")?;
    let (mut stream, _) = listener.accept()?;
    let serialized_data = unsafe { std::slice::from_raw_parts(samples as *const f32 as *const u8, std::mem::size_of::<f32>()) };
    stream.write_all(&serialized_data)?;
    Ok(())
}

#[derive(Parser, Debug)]
#[command(version, about = "CPAL feedback example", long_about = None)]
struct Opt {
    /// The input audio device to use
    #[arg(short, long, value_name = "IN", default_value_t = String::from("default"))]
    input_device: String,

    /// The output audio device to use
    #[arg(short, long, value_name = "OUT", default_value_t = String::from("default"))]
    output_device: String,

    /// Specify the delay between input and output
    #[arg(short, long, value_name = "DELAY_MS", default_value_t = 150.0)]
    latency: f32,

    // /// Use the JACK host
    // #[cfg(all(
    //     any(
    //         target_os = "linux",
    //         target_os = "dragonfly",
    //         target_os = "freebsd",
    //         target_os = "netbsd"
    //     ),
    //     feature = "jack"
    // ))]
    // #[arg(short, long)]
    // #[allow(dead_code)]
    // jack: bool,
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    // Input section
    let input_stream = setup_input(&opt)?;
    
    // Play the streams.
    println!(
        "Starting the input and output streams with `{}` milliseconds of latency.",
        opt.latency
    );
    input_stream.play()?;
    loop{
    }
}

fn setup_input(opt: &Opt) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();

    // Find input device.
    let input_device = if opt.input_device == "default" {
        host.default_input_device()
    } else {
        host.input_devices()?
            .find(|x| x.name().map(|y| y == opt.input_device).unwrap_or(false))
    }
    .expect("failed to find input device");

    println!("Using input device: \"{}\"", input_device.name()?);

    // Configure input stream.
    let config: cpal::StreamConfig = input_device.default_input_config()?.into();

    // Create delay buffer.
    let latency_frames = (opt.latency / 1_000.0) * config.sample_rate.0 as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;
    let ring = HeapRb::<f32>::new(latency_samples * 2);
    let (mut producer, mut _consumer) = ring.split();

    // Fill the delay buffer with 0.0.
    for _ in 0..latency_samples {
        producer.push(0.0).unwrap();
    }

    // Input data callback function.
    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        for &sample in data {
            let _ = send_audio_data(&sample);
        }
    };

    // Build input stream.
    println!(
        "Attempting to build input stream with f32 samples and `{:?}`.",
        config
    );
    let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None)?;
    println!("Successfully built input stream.");

    Ok(input_stream)
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}