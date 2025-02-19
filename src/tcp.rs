use std::{
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    os::fd::AsRawFd,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll},
    thread,
    time::Duration,
};

use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use crate::runtime::{executor::REACTOR, reactor::ReactorRegister};

struct AsyncTcpStream {
    inner: TcpStream,
}

impl AsyncTcpStream {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let sock_type = Type::STREAM;
        let protocol = Protocol::TCP;
        let socket_addr = addr.to_socket_addrs()?.next().unwrap();
        let domain = Domain::for_address(socket_addr);
        let socket = Socket::new(domain, sock_type, Some(protocol))
            .unwrap_or_else(|_| {
                panic!("Failed to create socket at address {}", socket_addr)
            });

        socket.set_nonblocking(true).unwrap();
        ConnectTcpStreamFuture {
            socket: &socket,
            addr: SockAddr::from(socket_addr),
        }
        .await?;
        let stream = TcpStream::from(socket);
        // 也许没必要,它可能跟随socket
        stream.set_nonblocking(true)?;
        Ok(Self { inner: stream })
    }
    pub async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        ReadTcpStreamFuture {
            buf,
            stream: &mut self.inner,
        }
        .await
    }
    pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        WriteTcpStreamFuture {
            buf,
            stream: &mut self.inner,
        }
        .await
    }
}

pub struct ConnectTcpStreamFuture<'a> {
    socket: &'a Socket,
    addr: SockAddr,
}
impl Future for ConnectTcpStreamFuture<'_> {
    type Output = Result<(), io::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.socket.connect(&self.addr) {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e)
                if e.raw_os_error() == Some(libc::EINPROGRESS)
                    || e.kind() == io::ErrorKind::WouldBlock =>
            {
                REACTOR
                    .sender
                    .send(ReactorRegister {
                        interest_fd: self.socket.as_raw_fd(),
                        events: (libc::EPOLLOUT) as u32,
                        waker: cx.waker().clone(),
                    })
                    .unwrap();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

pub struct ReadTcpStreamFuture<'a> {
    buf: &'a mut [u8],
    stream: &'a mut TcpStream,
}
impl Future for ReadTcpStreamFuture<'_> {
    type Output = io::Result<usize>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let mut vec = Vec::with_capacity(self.buf.len());
        unsafe { vec.set_len(self.buf.len()) };
        match self.stream.read(&mut vec) {
            Ok(n) => {
                self.buf.copy_from_slice(&mut vec[0..n]);
                Poll::Ready(Ok(n))
            }
            Err(e)
                if e.raw_os_error() == Some(libc::EINPROGRESS)
                    || e.kind() == io::ErrorKind::WouldBlock =>
            {
                REACTOR
                    .sender
                    .send(ReactorRegister {
                        interest_fd: self.stream.as_raw_fd(),
                        events: (libc::EPOLLIN) as u32,
                        waker: cx.waker().clone(),
                    })
                    .unwrap();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

pub struct WriteTcpStreamFuture<'a> {
    buf: &'a [u8],
    stream: &'a mut TcpStream,
}
impl Future for WriteTcpStreamFuture<'_> {
    type Output = io::Result<usize>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let buf = self.buf;
        match self.stream.write(buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e)
                if e.raw_os_error() == Some(libc::EINPROGRESS)
                    || e.kind() == io::ErrorKind::WouldBlock =>
            {
                REACTOR
                    .sender
                    .send(ReactorRegister {
                        interest_fd: self.stream.as_raw_fd(),
                        events: (libc::EPOLLOUT) as u32,
                        waker: cx.waker().clone(),
                    })
                    .unwrap();
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }
}

struct Sleep {
    duration: Duration,
    completed: Arc<AtomicBool>,
    // 保证线程只运行一次
    b_thread: bool,
}
impl Sleep {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            completed: Arc::new(AtomicBool::new(false)),
            b_thread: true,
        }
    }
}
impl Future for Sleep {
    type Output = u128;
    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        if self.completed.load(std::sync::atomic::Ordering::Relaxed) {
            Poll::Ready(self.duration.as_millis())
        } else {
            if self.b_thread {
                self.b_thread = false;
                let waker = cx.waker().clone();
                let duration = self.duration.clone();
                let completed = self.completed.clone();
                thread::spawn(move || {
                    thread::sleep(duration);
                    completed.store(true, Ordering::SeqCst);
                    waker.wake();
                });
            }
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod test_tcp {
    use std::time::Duration;

    use super::*;
    use crate::runtime::{
        executor::{Executor, NOW},
        reactor::IdManager,
        task::Task,
    };

    #[test]
    pub fn test_tcp() {
        let mut executor = Executor::new(10, 500);
        for i in 0..1000 {
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
                unsafe { buf.set_len(n) };
                
                println!(
                    "{}[time:{}] read[{}]:{:?}",
                    thread.name().unwrap(),
                    NOW.elapsed().as_millis(),
                    i,
                    buf
                );
            };
            let future = Box::pin(future);
            let task = Task {
                id: IdManager::INVALID_ID,
                future: future,
            };
            executor.add_task(task);
        }
        executor.block();
    }
}
