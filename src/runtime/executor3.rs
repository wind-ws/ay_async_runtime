// Executor 收到Future,将Future发送给线程池
// 线程池 接受 Future ,
// 并且将Future分配到 A线程中(合理分配任务,拒绝线程之间的任务窃取(有时间在搞吧)),
// A线程管理Future,
// 为Future分配A线程唯一id,将Future存入hashmap<id,future>
// 且将Future id放入执行队列(进行首次调用),
// 当A线程调用Future poll ,
// Ready则消耗Future和id,
// Pending时 将Future保存到hashmap中,等待waker调用
//
// waker包含id和sender,用于将id再次发送到 A线程的执行队列,
// 一般 waker由 Reactor的epoll调用,
// 在执行Future poll函数中,若返回Pending,要在之前将waker注册进入Reactor(waker被clone),
// 且你需要分配id,为epoll event所用

use core::task;
use std::{
    collections::HashMap,
    io,
    os::fd::{AsRawFd, RawFd},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64},
    },
    task::{Context, Wake, Waker},
    thread,
};

use crossbeam::{
    channel::{Sender, unbounded},
    epoch::Atomic,
};
use lazy_static::lazy_static;
use libc::exit;

use crate::epoll::{self, EpollEvent};

type MyFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type ID = u64;

lazy_static! {
    static ref REACTOR: Reactor = Reactor::new();
}

/// #done
struct Task {
    pub id: ID,
    pub future: MyFuture,
}
impl Task {
    pub fn waker(&self, sender: Sender<ID>) -> TaskWaker {
        TaskWaker {
            id: self.id,
            sender,
        }
    }
}
/// #done
struct TaskWaker {
    pub id: ID,
    pub sender: Sender<ID>,
}
impl Wake for TaskWaker {
    fn wake(self: std::sync::Arc<Self>) {
        self.sender.send(self.id).unwrap();
    }
}

struct WokerThread {
    pub thread_id: ID,
    pub task_sender: Sender<Task>,
    pub thread: thread::JoinHandle<()>,
    // /// 当前任务数量
    // amount:AtomicU32,
    // /// true:强制停止当前工作线程
    // stop:AtomicBool,
    // /// 线程空闲 死亡时间
    // idle_dead_ms:u64,
}
impl WokerThread {
    pub fn new(id: ID) -> Self {
        let (sender, receiver) = unbounded();
        let builder =
            thread::Builder::new().name(format!("work_thread[{}]", id));
        let thread = builder
            .spawn(move || {
                let mut map = HashMap::<ID, Task>::new();
                let mut id_manager = IdManager::new();
                let task_receiver: crossbeam::channel::Receiver<Task> =
                    receiver;
                let (id_sender, id_receiver) = unbounded::<ID>();
                loop {
                    // 接受外部的task
                    while let Ok(mut task) = task_receiver.try_recv() {
                        let id = id_manager.get_id();
                        // // id由线程内部管理,无论如何都需要改变它
                        task.id = id;
                        // 执行Future,Pending则放入map
                        let waker = Arc::new(task.waker(id_sender.clone()));
                        let mut waker = Waker::from(waker);
                        let mut cx = Context::from_waker(&mut waker);
                        match task.future.as_mut().poll(&mut cx) {
                            std::task::Poll::Ready(_) => {}
                            std::task::Poll::Pending => {
                                //pending 将task放入hashmap
                                map.insert(task.id, task);
                            }
                        }
                    }
                    // 执行Future
                    while let Ok(id) = id_receiver.try_recv() {
                        let task = map.get_mut(&id).unwrap();
                        let waker = Arc::new(task.waker(id_sender.clone()));
                        let mut waker = Waker::from(waker);
                        let mut cx = Context::from_waker(&mut waker);
                        match task.future.as_mut().poll(&mut cx) {
                            std::task::Poll::Ready(_) => {
                                // 移除 map的 task ,和id回收
                                map.remove(&id);
                            }
                            std::task::Poll::Pending => {}
                        }
                    }
                }
            })
            .unwrap();
        Self {
            thread_id: id,
            task_sender: sender,
            thread,
            // amount: AtomicU32::new(0),
            // stop: AtomicBool::new(false),
            // idle_dead_ms: 0,
        }
    }

    pub fn add_task(&self, task: Task) {
        self.task_sender.send(task).unwrap();
    }
}

/// 只管理线程,不管理任务
struct ThreadPool {
    pub woker: Vec<WokerThread>,
}
impl ThreadPool {
    /// `n`:创建n个工作线程
    pub fn new(n: usize) -> Self {
        let mut woker = Vec::with_capacity(n);
        for i in 0..n {
            woker.push(WokerThread::new(i as u64));
        }
        Self { woker }
    }
}

/// 分配任务 给其他线程
struct Executor {
    thread_pool: ThreadPool,
}
impl Executor {
    pub fn new(n: usize) -> Self {
        Self {
            thread_pool: ThreadPool::new(n),
        }
    }
    pub fn add_task(&self, task: Task) {
        let a = rand::random_range(0..self.thread_pool.woker.len());
        self.thread_pool.woker[a].add_task(task);
    }
}

/// #done
/// #rename
struct ReactorRegister {
    pub interest_fd: RawFd,
    pub events: u32,
    pub waker: Waker,
}

/// #done
struct Reactor {
    fd: RawFd,
    pub sender: Sender<ReactorRegister>,
    thread: thread::JoinHandle<()>,
}
impl Reactor {
    pub fn new() -> Self {
        let fd = epoll::create().unwrap();
        let (sender, receiver) = unbounded::<ReactorRegister>();
        let thread = thread::spawn(move || {
            let mut epoll_events: Vec<EpollEvent> = Vec::with_capacity(1024);
            let mut map = HashMap::<u64, ReactorRegister>::new();
            let mut id_manager = IdManager::new();
            loop {
                // epoll_events.clear();
                let n = epoll::wait(fd, &mut epoll_events, 1024, 0).unwrap();
                let n = n as usize;
                unsafe { epoll_events.set_len(n) };
                // 来自Future poll函数
                // 接受事件,并注册到epoll
                while let Ok(reg) = receiver.try_recv() {
                    let event_id = id_manager.get_id();
                    Reactor::register(
                        fd.clone(),
                        reg.events,
                        reg.interest_fd,
                        event_id,
                    )
                    .unwrap();
                    println!("insert:{}", event_id);
                    map.insert(event_id, reg);
                }
                // 被触发的事件id
                for event_id in &epoll_events[0..n] {
                    let event_id = event_id.u64;
                    // println!("happen:{}", event_id);
                    match map.remove(&event_id) {
                        Some(reg) => {
                            println!("remove:{}", event_id);

                            Reactor::unregister(
                                fd.clone(),
                                reg.events,
                                reg.interest_fd,
                                event_id,
                            )
                            .unwrap();
                            reg.waker.wake();
                            id_manager.recycle(event_id);
                        }
                        None => (),
                    }
                }
            }
        });
        Self { fd, sender, thread }
    }

    pub fn register(
        fd: RawFd,
        events: u32,
        interest_fd: RawFd,
        id: u64,
    ) -> io::Result<()> {
        let mut event = EpollEvent { events, u64: id };
        match epoll::ctl(fd, libc::EPOLL_CTL_ADD, interest_fd, &mut event) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                epoll::ctl(fd, libc::EPOLL_CTL_MOD, interest_fd, &mut event)
            }
            Err(e) => Err(e),
        }
    }

    pub fn unregister(
        fd: RawFd,
        events: u32,
        interest_fd: RawFd,
        id: u64,
    ) -> io::Result<()> {
        let mut event = EpollEvent { events, u64: id };
        match epoll::ctl(fd, libc::EPOLL_CTL_DEL, interest_fd, &mut event) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn register_event(&self, event: ReactorRegister) {
        self.sender.send(event).unwrap();
    }
}

/// #done
struct IdManager {
    count: ID,
    bin: Vec<ID>,
}
impl IdManager {
    pub const InValidId: ID = ID::MAX;
    pub fn new() -> Self {
        Self {
            count: 0,
            bin: Vec::new(),
        }
    }
    pub fn get_id(&mut self) -> ID {
        match self.bin.pop() {
            Some(id) => id,
            None => {
                let id = self.count;
                self.count += 1;
                id
            }
        }
    }
    pub fn recycle(&mut self, id: ID) {
        self.bin.push(id);
    }
}

mod tcp {
    use std::{
        io::{self, Read, Write},
        net::{TcpStream, ToSocketAddrs},
        pin::Pin,
        task::{Context, Poll},
    };

    use socket2::{Domain, Protocol, SockAddr, Socket, Type};

    use super::{REACTOR, ReactorRegister, *};
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
            ReadTcpStreamFuture { buf, stream: &mut self.inner }.await
        }
        pub async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            WriteTcpStreamFuture{ buf, stream: &mut self.inner }.await
        }
    }

    pub struct ConnectTcpStreamFuture<'a> {
        socket: &'a Socket,
        addr: SockAddr,
    }
    impl Future for ConnectTcpStreamFuture<'_> {
        type Output = Result<(), io::Error>;

        fn poll(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Self::Output> {
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

    pub struct WriteTcpStreamFuture<'a>{
        buf: &'a [u8],
        stream: &'a mut TcpStream,
    }
    impl Future for WriteTcpStreamFuture<'_> {
        type Output=io::Result<usize>;
    
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let buf = self.buf;
            match self.stream.write(buf) {
                Ok(n) => {
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
    #[cfg(test)]
    mod test_tcp {
        use std::time::Duration;

        use super::{super::*, *};
        #[test]
        pub fn test_tcp_read() {
            let executor = Executor::new(10);
            let future = async move {
                let mut stream = AsyncTcpStream::connect("127.0.0.1:3000").await.unwrap();
                let mut buf = Vec::<u8>::with_capacity(2048);
                stream.read(&mut buf).await.unwrap();
                println!("{:?}",buf);
                println!("{:?}",buf);
            };
            let future = Box::pin(future);
            let task = Task {
                id: IdManager::InValidId,
                future: future,
            };
            executor.add_task(task);

            thread::sleep(Duration::from_millis(3000));
        }
        
        #[test]
        pub fn test_tcp(){
            let executor = Executor::new(10);
            for i in 0..100 {
                let future = async move {
                    let mut stream = AsyncTcpStream::connect("127.0.0.1:3000").await.unwrap();
                    let mut buf = Vec::<u8>::with_capacity(2048);
                    for j in 0..10 {
                        buf.push(j);
                    }
                    buf[1] = i;
                    let n =stream.write(&buf).await.unwrap();
                    stream.read(&mut buf).await.unwrap();
                    println!("{:?}",buf);
                };
                let future = Box::pin(future);
                let task = Task {
                    id: IdManager::InValidId,
                    future: future,
                };
                executor.add_task(task);
            }
            thread::sleep(Duration::from_millis(2000));
        }
    }
}

mod test_executor3 {
    use std::{
        future::{self, Ready},
        net,
        process::exit,
        sync::atomic::{AtomicBool, Ordering},
        task::Poll,
        time::{Duration, Instant},
    };

    use socket2::{Domain, Protocol, SockAddr, Socket, Type};

    use super::*;

    lazy_static! {
        static ref NOW: Instant = Instant::now();
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

    #[test]
    fn test() {
        let future = async {
            println!("1");
            let now = Instant::now();
            let a = Sleep::new(Duration::from_millis(120)).await;
            println!("2:{}", now.elapsed().as_millis());
            Sleep::new(Duration::from_millis(150)).await;
            println!("3:{}", now.elapsed().as_millis());
        };
        let future = Box::pin(future);
        let worker = WokerThread::new(1);
        let task = Task {
            id: IdManager::InValidId,
            future: future,
        };
        worker.add_task(task);
        // thread::sleep(Duration::from_millis(100));
        thread::sleep(Duration::from_millis(500));
    }

    pub struct ConnectStreamFuture<'a> {
        socket: &'a Socket,
        addr: SockAddr,
    }
    impl Future for ConnectStreamFuture<'_> {
        type Output = Result<(), io::Error>;

        fn poll(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Self::Output> {
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
                            events: (libc::EPOLLIN | libc::EPOLLOUT) as u32,
                            waker: cx.waker().clone(),
                        })
                        .unwrap();
                    Poll::Pending
                }
                Err(e) => Poll::Ready(Err(e)),
            }
        }
    }

    pub struct TcpStream {
        inner: net::TcpStream,
    }
    impl TcpStream {
        pub async fn connect(
            addr_generator: impl net::ToSocketAddrs,
        ) -> io::Result<Self> {
            let sock_type = Type::STREAM;
            let protocol = Protocol::TCP;
            let socket_addr =
                addr_generator.to_socket_addrs().unwrap().next().unwrap();
            let domain = Domain::for_address(socket_addr);

            let socket = Socket::new(domain, sock_type, Some(protocol))
                .unwrap_or_else(|_| {
                    panic!("Failed to create socket at address {}", socket_addr)
                });
            socket.set_nonblocking(true).unwrap();

            ConnectStreamFuture {
                socket: &socket,
                addr: SockAddr::from(socket_addr),
            }
            .await?;
            let stream = net::TcpStream::from(socket);
            Ok(Self { inner: stream })
        }
    }

    #[test]
    fn test_reactor() {
        // 需要提前启动一个服务端
        let worker = WokerThread::new(1);
        for i in 0..100 {
            let future = async move {
                println!("{}start:{}", i.clone(), NOW.elapsed().as_millis());
                let addr = format!("127.0.0.1:3000");
                let stream = TcpStream::connect(addr).await.unwrap();
                Sleep::new(Duration::from_millis(100)).await;
                println!("{}done:{}", i.clone(), NOW.elapsed().as_millis());
            };
            let future = Box::pin(future);
            let task = Task {
                id: IdManager::InValidId,
                future: future,
            };
            worker.add_task(task);
        }
        thread::sleep(Duration::from_millis(3000));
    }

    #[test]
    fn test_executor() {
        let executor = Executor::new(10);
        for i in 0..1000 {
            let future = async move {
                let thread = thread::current();
                let name = thread.name().unwrap();
                println!(
                    "{} {}start:{}",
                    name,
                    i.clone(),
                    NOW.elapsed().as_millis()
                );
                let addr = format!("127.0.0.1:3000");
                let stream = TcpStream::connect(addr).await.unwrap();
                Sleep::new(Duration::from_millis(100)).await;
                println!(
                    "{} {}done:{}",
                    name,
                    i.clone(),
                    NOW.elapsed().as_millis()
                );
            };
            let future = Box::pin(future);
            let task = Task {
                id: IdManager::InValidId,
                future: future,
            };
            executor.add_task(task);
        }
        thread::sleep(Duration::from_millis(3000));
    }
}
