//! 在 Future执行过程中无法推进需要等待时，需要将 Executor提供的 Waker注册在 Reactor上,
//! Reactor负责监听 Future是否Ready,
//! 如果Ready，则通过 Waker通知 Executor继续执行对应的 Future

use std::{cell::UnsafeCell, io, os::fd::RawFd, sync::Arc, thread};

use crossbeam::channel::Sender;

use super::task::{ID, Task};

pub struct Reactor {
    // /// 将task 发送给 executor
    // ///
    // /// id通过epoll event 获得
    // ready_queue: Sender<ID>,

    // /// 用于调用wake
    // future_list: Arc<Vec<UnsafeCell<Option<Task>>>>,
    // #wait: 也许Reactor需要一个单独的线程去监听Event
    thread: std::thread::JoinHandle<()>,
}
impl Reactor {
    pub fn new(ready_queue: Sender<ID>) -> Self {
        let thread = thread::spawn(move|| {
            // future_list;
            // ...
        });
        Self { thread }
    }
}

/// fd拥有者\
/// 用于创建和销毁epoll实例
struct FdOwner {
    fd: RawFd,
}
impl FdOwner {
    pub fn new() -> io::Result<Self> {
        match crate::epoll::create() {
            Ok(fd) => Ok(FdOwner { fd }),
            Err(e) => Err(e),
        }
    }
}
impl Drop for FdOwner {
    fn drop(&mut self) {
        crate::epoll::close(self.fd).unwrap();
    }
}
/// fd使用者
struct FdUser {
    fd: FdOwner,
}
