use std::{io, os::fd::RawFd, task::Waker, thread};

use crossbeam::channel::{Sender, unbounded};
use fxhash::FxHashMap;

use super::executor::ID;
use crate::epoll::{self, EpollEvent};

/// #done
/// #rename
pub struct ReactorRegister {
    pub interest_fd: RawFd,
    pub events: u32,
    pub waker: Waker,
}

/// #done
pub struct Reactor {
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
            let mut map = FxHashMap::<u64, ReactorRegister>::default();
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
                    // println!("insert:{}", event_id);
                    map.insert(event_id, reg);
                }
                // 被触发的事件id
                for event_id in &epoll_events[0..n] {
                    let event_id = event_id.u64;
                    // println!("happen:{}", event_id);
                    match map.remove(&event_id) {
                        Some(reg) => {
                            // println!("remove:{}", event_id);

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
pub struct IdManager {
    count: ID,
    bin: Vec<ID>,
}
impl IdManager {
    pub const INVALID_ID: ID = ID::MAX;
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
