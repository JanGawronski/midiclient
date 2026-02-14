use std::{
    error::Error,
    net::UdpSocket,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::Duration,
};

use clap::Parser;
use midir::{MidiInput, MidiOutput};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    address: String,

    #[arg(value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,

    #[arg(short, long)]
    inputs_to_ignore: Vec<u8>,

    #[arg(short, long)]
    output_to_forward: Option<u8>,
}

fn main() {
    let args = Args::parse();

    match run(
        args.address,
        args.port,
        args.inputs_to_ignore,
        args.output_to_forward,
    ) {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run(
    address: String,
    port: u16,
    inputs_to_ignore: Vec<u8>,
    output_to_forward: Option<u8>,
) -> Result<(), Box<dyn Error>> {
    let midi_in = MidiInput::new("MIDI client input")?;
    let midi_out = MidiOutput::new("MIDI client output")?;

    println!("Available input ports:");
    for (i, p) in midi_in.ports().iter().enumerate() {
        println!("{}: {} (ID: \"{}\")", i, midi_in.port_name(p)?, p.id());
    }

    println!("\nAvailable output ports:");
    for (i, p) in midi_out.ports().iter().enumerate() {
        println!("{}: {} (ID: \"{}\")", i, midi_out.port_name(p)?, p.id());
    }

    let out_port = {
        let ports = midi_out.ports();
        if let Some(port_id) = output_to_forward {
            ports
                .get(port_id as usize)
                .expect("Incorect output port number")
        } else {
            ports.last().unwrap()
        }
        .to_owned()
    };

    let (tx, rx) = channel();

    let _inputs = midi_in
        .ports()
        .iter()
        .enumerate()
        .filter_map(|(id, port)| {
            if !inputs_to_ignore.contains(&(id as u8)) {
                let local_tx = tx.clone();
                let midi_in_temp = MidiInput::new(
                    format!(
                        "MIDI input for reading {}",
                        midi_in.port_name(&port).unwrap()
                    )
                    .as_str(),
                )
                .unwrap();
                midi_in_temp
                    .connect(
                        &port,
                        "midir-forward",
                        move |_stamp, message, _| {
                            let _ = local_tx.send((message.to_owned(), true));
                        },
                        (),
                    )
                    .ok()
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let mut conn_out = midi_out.connect(&out_port, "midir-forward")?;

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(format!("{address}:{port}"))?;

    let recv_socket = socket.try_clone()?;
    spawn(move || loop {
        let mut buf: [u8; 64] = [0; 64];
        let number_of_bytes = recv_socket.recv(&mut buf).unwrap();
        let _ = tx.send(((&mut buf[..number_of_bytes]).to_vec(), false));
    });

    let keep_alive_socket = socket.try_clone()?;
    spawn(move || loop {
        let _ = keep_alive_socket.send(&[]);
        sleep(Duration::from_secs(10));
    });

    for (message, is_local) in rx {
        println!("{:?}", message);
        let _ = conn_out.send(&message);
        if is_local {
            let _ = socket.send(&message);
        }
    }

    Ok(())
}
