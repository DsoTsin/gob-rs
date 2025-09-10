fn main() {
    let data = std::fs::read("goth-session.bin").unwrap();
    for (i, chunk) in data.chunks(16).enumerate() {
        print!("{:04x}: ", i * 16);
        for b in chunk {
            print!("{:02x} ", b);
        }
        print!(" | ");
        for b in chunk {
             if *b >= 32 && *b < 127 {
                 print!("{}", *b as char);
             } else {
                 print!(".");
             }
        }
        println!();
    }
}

