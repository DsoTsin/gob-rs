use gobx::{Decoder, Encoder, Gob, GobDecodable};
use std::env;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::process;

#[Gob(id = 64, interpret_as = "map[interface{}]interface{}")]
#[derive(Debug, Default)]
struct UserInfo {
    uid: i64,
    uname: String,
    email: String,
    #[gob(name="_old_uid")] // Not supported by current macro
    old_uid: String,
    #[gob(name="userHasTwoFactorAuth")]
    two_factor_auth: bool,
}

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

    println!("Decoding {}...", filename);
    let mut reader = BufReader::new(file);
    
    // Read original bytes for comparison
    let mut original_bytes = Vec::new();
    reader.read_to_end(&mut original_bytes).unwrap();
    
    // Reset reader for decoding
    reader.get_mut().seek(std::io::SeekFrom::Start(0)).unwrap();
    let mut decoder = Decoder::new(reader);
    
    // We will collect values to re-encode them
    let mut values = Vec::new();

    loop {
        match decoder.read_next() {
            Ok(Some(v)) => {
                println!("Decoded Value: {:?}", v);
                values.push(v);
            }
            Ok(None) => break,
            Err(e) => {
                eprintln!("Decoder error: {:?}", e);
                // We might want to stop or continue depending on error
                // For goth-session.bin, we fixed the EOF error, so it should finish cleanly.
                break;
            }
        }
    }
    
    // Test Encoding (Round Trip) for supported types
    println!("\n--- Testing Encoder ---");
    let mut buffer = Vec::new();
    let mut encoder = Encoder::new(&mut buffer);
    
    for v in &values {
        // We only test primitive encoding for now via Value::encode
        // This won't produce a full valid Gob stream (missing TypeIDs/Length headers for top-level messages)
        // But verifies the content encoding logic.
        match v.encode(&mut encoder) {
            Ok(_) => println!("Encoded successfully: {:?}", v),
            Err(e) => println!("Skipping encoding for {:?}: {}", v, e),
        }
    }
    
    if !buffer.is_empty() {
        println!("Encoded {} bytes.", buffer.len());
        // Hex dump first few bytes
        println!("Encoded Hex:  {:?}", &buffer[..std::cmp::min(buffer.len(), 20)]);
        println!("Original Hex: {:?}", &original_bytes[..std::cmp::min(original_bytes.len(), 20)]);
        
        if buffer.len() == original_bytes.len() {
             if buffer == original_bytes {
                 println!("SUCCESS: Encoded bytes match original file exactly!");
             } else {
                 println!("WARNING: Byte mismatch despite same length.");
             }
        } else {
             println!("WARNING: Length mismatch (Encoded: {}, Original: {})", buffer.len(), original_bytes.len());
             println!("Note: This is expected if the Encoder implementation is partial (missing Type definitions/headers).");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redis::Commands;
    
    #[test]
    fn test_decode_user_info() {
        let client = redis::Client::open("redis://cdn.mixstudio.tech:30002/0").unwrap();
        let mut con = client.get_connection().unwrap();
        //let _: () = con.set("test_key", "test_value").unwrap();
        let buffer: Vec<u8> = con.get("aaac32bd1d759408").unwrap();
        std::fs::write("normal-session-2.bin", &buffer).unwrap();
        //assert_eq!(value, "test_value");
        // let filename = "normal-session.bin";
        // let mut file = File::open(filename).expect("Failed to open normal-session.bin");
        // let mut buffer = Vec::new();
        // file.read_to_end(&mut buffer).expect("Failed to read file");
        let cursor = std::io::Cursor::new(&buffer);
        let mut decoder = Decoder::new(cursor);
        //println!("Test: Decoding generic values from {}", filename);
        let user_info: UserInfo = decoder.decode_into().expect("Failed to decode UserInfo");
        println!("Decoded UserInfo: {:?}", user_info);
        assert_eq!(user_info.uid, 1);
        assert_eq!(user_info.uname, "dsotsen");
        assert_eq!(user_info.old_uid, "1");
        assert_eq!(user_info.two_factor_auth, false);
    }

    #[test]
    fn test_encode_user_info() {
        //let client = redis::Client::open("redis://cdn.mixstudio.tech:30002/0").unwrap();
        //let mut con = client.get_connection().unwrap();
        //
        //let buffer: Vec<u8> = con.get("aaac32bd1d759408").unwrap();
        let user_info = UserInfo {
            uname: "dsotsen".to_string(),
            email: "dsotsen@qq.com".to_string(),
            two_factor_auth: false,
            old_uid: "1".to_string(),
            uid: 1,
        };
        // Test basic encoding works (doesn't crash)
        let mut buffer = Vec::new();
        let mut encoder = Encoder::new(&mut buffer);
        
        // Note: UserInfo.encode() currently encodes as struct (field deltas), not as map
        // even though it has interpret_as="map[...]". The encode side needs more work.
        // For now, just verify it doesn't crash.
        user_info.encode(&mut encoder).expect("Failed to encode UserInfo");
        
        // Verify we got some data
        assert!(!buffer.is_empty(), "Encoded buffer should not be empty");
        // let _: () = con.set("aaac32bd1d759409", &buffer).unwrap();
        println!("Encoded UserInfo to {} bytes", buffer.len());
        
        let file_buffer = std::fs::read("normal-session-2.bin").unwrap();
        assert_eq!(buffer, file_buffer);
    }
}
