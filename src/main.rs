// use tokio::{
//     io::{AsyncReadExt, AsyncWriteExt},
//     net::TcpListener,
// };

// #[tokio::main(flavor = "multi_thread", worker_threads = 10)]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let listener = TcpListener::bind("127.0.0.1:3000").await?;
//     let mut _count = 0;
//     loop {
//         let (mut socket, _) = listener.accept().await?;
//         tokio::spawn(async move {
//             let mut buf = [0; 1024];
//             loop {
//                 let n = match socket.read(&mut buf).await {
//                     Ok(0) => return,
//                     Ok(n) => n,
//                     Err(e) => {
//                         eprintln!("failed to read from socket; err = {:?}", e);
//                         return;
//                     }
//                 };
//                 if let Err(e) = socket.write_all(&buf[0..n]).await {
//                     eprintln!("failed to write to socket; err = {:?}", e);
//                     return;
//                 }
//             }
//         });
//     }
// }

// use std::{thread, time::Duration};

// use interview_rust_project::runtime::executor::NOW;
// use tokio::{
//     io::{AsyncReadExt, AsyncWriteExt},
//     net::TcpStream,
//     stream,
// };

// // 最低: 879ms
// // 最高: 1738ms
// // 平均: 1250ms
// #[tokio::main]
// async fn main() {
//     for i in 0..10000 {
//         // println!("{}",i);
//         tokio::spawn(async move {
//             let mut stream =
//                 TcpStream::connect("127.0.0.1:3000").await.unwrap();

//             let mut buf = [0; 1024];
//             buf[0] = i as u8;
//             if let Err(e) = stream.write_all(&buf[0..10]).await {
//                 eprintln!("failed to write to socket; err = {:?}", e);
//                 return;
//             }
//             let n = match stream.read(&mut buf).await {
//                 Ok(0) => return,
//                 Ok(n) => n,
//                 Err(e) => {
//                     eprintln!("failed to read from socket; err = {:?}", e);
//                     return;
//                 }
//             };
//             let thread = thread::current();
//             println!(
//                 "{}[time:{}] read[{}]:{:?}",
//                 thread.name().unwrap(),
//                 NOW.elapsed().as_millis(),
//                 i,
//                 &buf[0..n]
//             );
//         });
//     }
//     tokio::time::sleep(Duration::from_millis(2000)).await;
// }

use std::thread;

use interview_rust_project::{
    runtime::executor::{Executor, NOW},
    tcp::AsyncTcpStream,
};
use proc_macro::{ay_main, ay_test};

// 最高: 3313ms
// 最低: 1438ms
// 平均: 2800ms
#[ay_main(worker_threads = 10)]
async fn main() {
    for i in 0..10000 {
        let future = async move {
            let thread = thread::current();
            let mut stream =
                AsyncTcpStream::connect("127.0.0.1:3000").await.unwrap();
            let mut buf = [0;1028];
            buf[0] = i as u8;
            let n = stream.write(&buf[0..10]).await.unwrap();
            let n = stream.read(&mut buf[0..n]).await.unwrap();
            println!(
                "{}[time:{}] read[{}]:{:?}",
                thread.name().unwrap(),
                NOW.elapsed().as_millis(),
                i,
                &buf[0..n]
            );
        };
        Executor::spawn(future);
    }
}

#[ay_test(worker_threads = 10)]
async fn a() {
    for i in 0..10000 {
        let future = async move {
            let thread = thread::current();
            let mut stream =
                AsyncTcpStream::connect("127.0.0.1:3000").await.unwrap();
            let mut buf = Vec::<u8>::with_capacity(2048);
            for j in 0..10 {
                buf.push(j);
            }
            buf[1] = i as u8;
            let n = stream.write(&buf).await.unwrap();
            unsafe { buf.set_len(n) };
            let n = stream.read(&mut buf).await.unwrap();

            println!(
                "{}[time:{}] read[{}]:{:?}",
                thread.name().unwrap(),
                NOW.elapsed().as_millis(),
                i,
                buf
            );
        };
        Executor::spawn(future);
    }
}
