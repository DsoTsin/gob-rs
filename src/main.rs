use gob_rs::Decoder;
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <gob_file>", args[0]);
        process::exit(1);
    }

    let filename = &args[1];
    let file = File::open(filename).unwrap_or_else(|err| {
        eprintln!("Error opening file {}: {}", filename, err);
        process::exit(1);
    });

    let reader = BufReader::new(file);
    let mut decoder = Decoder::new(reader);
    if let Err(e) = decoder.parse() {
        if e.kind() != std::io::ErrorKind::UnexpectedEof {
            eprintln!("Decoder error: {:?}", e);
        }
    }
}
