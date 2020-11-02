use std::io::{BufReader, BufRead};
use std::net::{TcpStream};
use std::io::Write;

/// Write a string messsage to a TcpStream
pub fn write_to_stream(stream : &mut TcpStream, msg : &String) {
    println!("Writing message to stream {}", msg);
    writeln!(stream, "{}", msg)
        .expect("Failed to write to TCPStream");
}

// Read a messag from the a TcpStream
pub fn read_from_stream(stream : &mut TcpStream) -> String {
    let mut reader = BufReader::new(stream);
    let mut msg = String::new();
    let len = reader.read_line(&mut msg)
        .expect("Failed to read message from stream");
    msg.truncate(len -1);
    println!("Read message from stream {}", msg);
    msg
}