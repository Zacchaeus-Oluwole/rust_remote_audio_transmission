// ======================  THE CLIENT SIDE OF THE REMOTE AUDIO TRANSMISSION ==================================

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;

use std::net::TcpStream;
use std::io::Read;

// Sample audio data type
type AudioSample = f32;

// // Deserialize function to convert bytes back to audio samples
// fn deserialize_audio(bytes: &[u8]) -> AudioSample {
//     bincode::deserialize(bytes).unwrap()
// }

// Function to receive audio data over TCP
fn receive_audio_data() -> std::io::Result<AudioSample> {
    let mut stream = TcpStream::connect("localhost:1234")?;

    let mut buf: [u8; 4] = [0; 4]; // Mutable byte array of size 4, initialized with zeros
    let buf_mut: &mut [u8] = &mut buf;
    
    let _ = stream.read_exact(buf_mut);

    let mut value_bytes: [u8; 4] = [0; 4]; // f32 is 4 bytes long
    value_bytes.copy_from_slice(buf_mut);
    let restored_float: f32 = unsafe { std::mem::transmute::<[u8; 4], f32>(value_bytes) };

    // let samples = deserialize_audio(&buffer);
    Ok(restored_float)
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

    // Output section
    let _output_stream = setup_output(&opt)?;

    // Play the streams.
    println!(
        "Starting the input and output streams with `{}` milliseconds of latency.",
        opt.latency
    );
    
    loop{

    }
}

fn setup_output(opt: &Opt) -> anyhow::Result<cpal::Stream> {
    let host = cpal::default_host();

    // Find output device.
    let output_device = if opt.output_device == "default" {
        host.default_output_device()
    } else {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == opt.output_device).unwrap_or(false))
    }
    .expect("failed to find output device");

    // println!("Using output device: \"{}\"", output_device.name()?);

    //  Output stream configuration.
    let config: cpal::StreamConfig = output_device.default_input_config()?.into();

    // Create delay buffer.
    let latency_frames = (opt.latency / 1_000.0) * config.sample_rate.0 as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;
    let ring = HeapRb::<f32>::new(latency_samples * 2);
    let (mut producer, mut consumer) = ring.split();

    // Fill the delay buffer with 0.0.
    for _ in 0..latency_samples {
        producer.push(0.0).unwrap();
    }

    // Output data callback function.
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {

        let received_samples = receive_audio_data().expect("Error: unable to receive audio data");
        
        for sample in data {

            let _ = producer.push(received_samples);
            
            *sample = match &consumer.pop() {
                Some(s) => *s,
                None => {
                    // input_fell_behind = true;
                    0.0
                }
            };
            // println!("Sample data: {:?}", &sample);
        }
    };

    // Build output stream.
    // println!(
    //     "Attempting to build output stream with f32 samples and `{:?}`.",
    //     config
    // );
    let output_stream = output_device.build_output_stream(&config, output_data_fn, err_fn, None)?;
    // println!("Successfully built output stream.");
    output_stream.play()?;

    Ok(output_stream)
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}