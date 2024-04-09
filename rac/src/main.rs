// // ======================  THE CLIENT SIDE OF THE REMOTE AUDIO TRANSMISSION ==================================

// client.rs

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use std::net::TcpStream;
use std::io::{Read, BufReader};

// Sample audio data type
type AudioSample = f32;

// Function to receive audio data over TCP
fn receive_audio_data(mut stream: TcpStream) -> std::io::Result<AudioSample> {
    let mut reader = BufReader::new(&mut stream);
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    let restored_float: f32 = unsafe { std::mem::transmute::<[u8; 4], f32>(buf) };
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
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    // Establish TCP connection
    let stream = TcpStream::connect("localhost:1234")?;
    
    // Output section
    let _output_stream = setup_output(&opt, stream)?;

    // Play the streams.
    println!(
        "Starting the input and output streams with `{}` milliseconds of latency.",
        opt.latency
    );
    
    loop {}
}

fn setup_output(opt: &Opt, stream: TcpStream) -> anyhow::Result<cpal::Stream> {
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
    let (mut producer, mut _consumer) = ring.split();

    // Fill the delay buffer with 0.0.
    for _ in 0..latency_samples {
        producer.push(0.0).unwrap();
    }
    let mut vec_value = Vec::<f32>::new();

    // Output data callback function.
    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {

        let received_samples = receive_audio_data(stream.try_clone().unwrap()).expect("Error: unable to receive audio data");
        
        // let mut vec_value = Vec::<f32>::new();
        for sample in data {

            let _ = vec_value.push(received_samples);
            
            *sample = match &vec_value.pop() {
                Some(s) => *s,
                None => {
                    // input_fell_behind = true;
                    0.0
                }
            };
            // println!("Sample data: {:?}", &sample);
        }
        // for sample in data {

        //     let _ = producer.push(received_samples);
            
        //     *sample = match &consumer.pop() {
        //         Some(s) => *s,
        //         None => {
        //             // input_fell_behind = true;
        //             0.0
        //         }
        //     };
        //     // println!("Sample data: {:?}", &sample);
        // }
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