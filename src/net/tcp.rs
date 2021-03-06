use std::{io::{BufReader, BufRead}, thread::sleep, time::Duration};
use std::net::{TcpStream};
use std::io::Write;


/// Write a string message to a TcpStream
/// Expects to receive an ACK message from the receiver
///
/// # Arguments
///
/// * `stream` TCP channel to exchange message bidirectionally
/// * `msg` the message to write
pub fn send_msg(stream : &mut TcpStream, msg : &String) {
    // To each message we excpect an acknowlege response
    println!("Writing message to stream {}", msg);
    write_to_stream(stream, msg).unwrap();
    println!("Waiting for ACK reply");
    let r = read_from_stream(stream).unwrap();
    assert_eq!(r, "ACK");
    println!("Read ACK reply")
}


/// Read a messag from the a TcpStream
/// Will return an ACK message to the sender
///
/// # Arguments
///
/// * `stream` TCP channel to exchange messages bidirectionally
pub fn receive_msg(stream : &mut TcpStream) -> String {
    println!("Wating for a message on stream...");
    let msg = read_from_stream(stream).unwrap();
    println!("Read message from stream {}", msg);
    write_to_stream(stream, &String::from("ACK")).unwrap();
    println!("Written ACK reply");
    msg
}

fn write_to_stream(stream : &mut TcpStream, msg : &String) -> Result<(), String> {
    match writeln!(stream, "{}", msg) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string())
    }
}

fn read_from_stream(stream : &mut TcpStream) -> Result<String, String> {
    let mut reader = BufReader::new(stream);
    let mut msg = String::new();
    match reader.read_line(&mut msg) {
        Ok(len) => {    
            msg.truncate(len -1);
            Ok(msg)
        },
        Err(e) => {
            Err(e.to_string())
        }
    }
}