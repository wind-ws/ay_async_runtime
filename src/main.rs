// use std::collections::HashMap;

// use axum::{extract::Query, routing::get, Router};

// #[tokio::main]
// async fn main() {
//     let app = Router::new().route(
//         "/",
//         get(f),
//     );

//     let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
//     axum::serve(listener, app).await.unwrap();
// }

use std::{
    io::{Read, Write},
    net::TcpListener, ops::Add,
};

// async fn f(Query(params): Query<HashMap<String, String>>)->String{
//     println!("{:#?}",params);
//     format!("{:#?}",params)
// }
fn main() {
    let listener = TcpListener::bind("127.0.0.1:3000").unwrap();
    let mut buf = Vec::<u8>::with_capacity(2024);
    let mut count = 0;
    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        unsafe { buf.set_len(2024) };
        let n = stream.read(&mut buf).unwrap();
        println!("len:{}", n);
        println!("{:?}", &buf[0..n]);
        buf[0] = count as u8;
        count += 1;
        unsafe { buf.set_len(n) };
        stream.write(&buf).unwrap();
    }
}
