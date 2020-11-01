use std::io::BufReader;
use std::net::{TcpStream};
use std::io::Write;
use std::io::Read;

/// Write a string messsage to a TcpStream
pub fn write_to_stream(stream : &mut TcpStream, msg : &String) {
    println!("Writing message to stream {}", msg);
    stream.write(msg.as_bytes())
        .expect("Failed to write message to stream");
}

// Read a messag from the a TcpStream
pub fn read_from_stream(stream : &mut TcpStream) -> String {
    let mut reader = BufReader::new(stream);
    let mut msg = String::new();
    reader.read_to_string(&mut msg).unwrap();
    println!("Read message from stream {}", msg);
    msg
}