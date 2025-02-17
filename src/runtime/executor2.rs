use std::{
    collections::HashMap,
    io,
    os::fd::{AsRawFd, RawFd},
    pin::{pin, Pin},
    process::Output,
    sync::Arc,
    task::{Context, Wake, Waker},
    thread, time::Duration,
};

use crossbeam::{
    epoch::{self, Atomic},
    queue::SegQueue,
};
use lazy_static::lazy_static;

use super::pool::{self, ThreadPool};
use crate::epoll::{self, EpollEvent};

static QUEUE: SegQueue<Task> = SegQueue::new();

lazy_static! {
    static ref REACTOR: Reactor = Reactor::new();
}

type MyFuture = Pin<Box<dyn Future<Output = ()> + Send>>;

struct Reactor {
    queue: Arc<SegQueue<(usize, Task)>>,
    /// 不使用epoll触发的事件
    event_queue: Arc<SegQueue<usize>>,
    pub fd: RawFd,
    thread: thread::JoinHandle<()>,
}
impl Reactor {
    pub fn new() -> Self {
        let fd = epoll::create().unwrap();
        let queue = Arc::new(SegQueue::new());
        let queue_ = queue.clone();
        let event_queue = Arc::new(SegQueue::new());
        let event_queue_ = event_queue.clone();
        let thread = thread::spawn(move || {
            let mut events: Vec<EpollEvent> = Vec::with_capacity(1024);
            let mut map = HashMap::<usize, Task>::new();
            let mut id_events: Vec<usize> = Vec::with_capacity(128);
            loop {
                events.clear();
                id_events.clear();
                // 捕捉事件
                let n = epoll::wait(fd, &mut events, 1024, 0).unwrap();
                let n = n as usize;
                while let Some(id) = event_queue_.pop() {
                    id_events.push(id);
                }
                thread::sleep(Duration::from_millis(10));
                while let Some((id, task)) = queue_.pop() {
                    map.insert(id, task);
                }
                for event in &events[0..n] {
                    let id = event.u64 as usize;
                    // 若事件id在哈希表中存在,则将Task发到QUEUE里
                    if let Some(task) = map.remove(&id) {
                        QUEUE.push(task);
                    }
                }
                for id in &id_events {
                    // 若事件id在哈希表中存在,则将Task发到QUEUE里
                    if let Some(task) = map.remove(id) {
                        QUEUE.push(task);
                    }
                }
            }
        });
        Self {
            queue,
            event_queue,
            fd,
            thread,
        }
    }

    pub fn register(
        &self,
        interest: &impl AsRawFd,
        id: usize,
    ) -> io::Result<()> {
        let fd = interest.as_raw_fd();
        let mut event = EpollEvent {
            events: (libc::EPOLLIN | libc::EPOLLONESHOT) as u32,
            u64: id as u64,
        };
        match epoll::ctl(self.fd, libc::EPOLL_CTL_ADD, fd, &mut event) {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                // one-shot epoll consumed, so needs to be re-armed.
                epoll::ctl(self.fd, libc::EPOLL_CTL_MOD, fd, &mut event)
            }
            Err(e) => Err(e),
        }
    }
    pub fn unregister(
        &self,
        interest: &impl AsRawFd,
        id: usize,
    ) -> io::Result<()> {
        let fd = interest.as_raw_fd();
        let mut event = EpollEvent {
            events: (libc::EPOLLIN | libc::EPOLLONESHOT) as u32,
            u64: id as u64,
        };
        match epoll::ctl(self.fd, libc::EPOLL_CTL_DEL, fd, &mut event) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub fn get_id() {}
}
unsafe impl Sync for Reactor {}

struct Task {
    id: usize,
    future: MyFuture,
}

struct Executor {
    thread_pool: ThreadPool,
}

impl Executor {
    fn run(&mut self) {
        loop {
            match QUEUE.pop() {
                Some(mut task) => {
                    let res = self.thread_pool.execute(move || {
                        let mut waker = Waker::noop();
                        let mut cx = Context::from_waker(&mut waker);
                        match task.future.as_mut().poll(&mut cx) {
                            std::task::Poll::Ready(_) => (),
                            std::task::Poll::Pending => {
                                REACTOR.queue.push((task.id, task));
                            }
                        }
                    });
                }
                None => {

                }
            }
        }
    }
}

#[cfg(test)]
mod tests_executor2 {
    use std::{
        sync::atomic::{AtomicBool, AtomicUsize},
        task::Poll,
        time::Duration,
    };

    use super::*;
    use crate::runtime::pool::{Config, State};

    struct Sleep {
        id: usize,
        time_ms: u64,
        over: Arc<AtomicBool>,
    }
    impl Sleep {
        pub fn new(id: usize, time_ms: u64) -> Self {
            Self {
                id,
                time_ms,
                over: Arc::new(AtomicBool::new(false)),
            }
        }
    }
    impl Future for Sleep {
        type Output = ();

        fn poll(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            if self.over.load(std::sync::atomic::Ordering::Relaxed) {
                Poll::Ready(())
            } else {
                let id = self.id;
                let ms = self.time_ms;
                let over = self.over.clone();
                let event_queue = REACTOR.event_queue.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(ms));
                    over.store(true, std::sync::atomic::Ordering::SeqCst);
                    event_queue.push(id);
                });
                Poll::Pending
            }
        }
    }

    static A: AtomicUsize = AtomicUsize::new(0);
    #[test]
    fn test() {
        let mut executor = Executor {
            thread_pool: ThreadPool::new(
                State::new(),
                Config::new(10, 500, 99999),
                5,
            ),
        };
        let future = async {
            A.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Sleep::new(1, 1000).await;
            A.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            println!("asbc");
        };

        QUEUE.push(Task {
            id: 1,
            future: Box::pin(future),
        });
        executor.run();
    }
}
