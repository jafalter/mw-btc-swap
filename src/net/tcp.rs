use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::io::Write;
use std::io::Read;

/// Write a string messsage to a TcpStream
pub fn write_to_stream(mut stream : &mut TcpStream, msg : &String) {
    stream.write(msg.as_bytes());
}

// Read a messag from the a TcpStream
pub fn read_from_stream(stream : &mut TcpStream) -> String {
    let mut reader = BufReader::new(stream);
    let mut msg = String::new();
    reader.read_to_string(&mut msg).unwrap();
    msg
}