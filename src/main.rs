use std::{
    io::{Read, Write},
    net::TcpListener,
};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:3000").unwrap();
    let mut buf = Vec::<u8>::with_capacity(2024);
    let mut count = 0;
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        unsafe { buf.set_len(2024) };
        let n = stream.read(&mut buf).unwrap();
        println!("read:{:?}", &buf[0..n]);
        buf[0] = count as u8;
        count += 1;
        unsafe { buf.set_len(n) };
        println!("write:{:?}\n", &buf[0..n]);
        stream.write(&buf).unwrap();
    }
}
