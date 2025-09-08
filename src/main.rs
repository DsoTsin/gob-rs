use gob_rs::Decoder;


fn main() {
    let mut file = std::fs::File::open("test.bin").unwrap();
    let mut reader = std::io::BufReader::new(file);
    let mut decoder = Decoder::new(reader);
    decoder.parse().unwrap();
}
