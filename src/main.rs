use std::{error::Error, net::UdpSocket, sync::mpsc::channel, thread::spawn};

use midir::{Ignore, MidiInput, MidiOutput, MidiOutputPort};

fn main() {
    match run() {
        Ok(_) => (),
        Err(err) => println!("Error: {}", err),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
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

    let connections = midi_in
        .ports()
        .iter()
        .map(|port| {
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
                        move |stamp, message, _| {
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

    spawn(move || {
        let socket = UdpSocket::bind("127.0.0.1:50000").unwrap();
        loop {
            let mut buf: [u8; 64] = [0; 64];
            let (number_of_bytes, _) = socket.recv_from(&mut buf).unwrap();
            let _ = tx.send(((&mut buf[..number_of_bytes]).to_vec(), false));
        }
    });

    let socket = UdpSocket::bind("127.0.0.1:50001")?;
    for (message, is_local) in rx {
        let _ = conn_out.send(&message);
        if is_local {
            let _ = socket.send_to(&message, "127.0.0.1:50000");
        }
    }

    Ok(())
}
