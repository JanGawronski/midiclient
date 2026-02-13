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
}

fn main() {
    let args = Args::parse();

    match run(args.address, args.port) {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run(address: String, port: u16) -> Result<(), Box<dyn Error>> {
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
        ports.last().unwrap().to_owned()
    };

    let mut conn_out = midi_out.connect(&out_port, "midir-forward")?;

    let (tx, rx) = channel();

    let _inputs = midi_in
        .ports()
        .iter()
        .filter_map(|port| {
            let port_name = midi_in.port_name(&port).unwrap();
            if !port_name.contains("midir-forward") {
                let local_tx = tx.clone();
                let midi_in_temp =
                    MidiInput::new(format!("MIDI input for reading {}", port_name).as_str())
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

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(format!("{address}:{port}"))?;

    let output_multicast_socket = UdpSocket::bind("0.0.0.0:0")?;
    output_multicast_socket.connect("225.0.0.37:21928")?;

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
        let _ = output_multicast_socket.send(&message);
        if is_local {
            let _ = socket.send(&message);
        }
    }

    Ok(())
}
