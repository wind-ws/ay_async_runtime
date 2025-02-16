//! 用户构造出的 Future最终需要提交到 Executor中执行

use std::{
    cell::UnsafeCell,
    sync::{Arc, atomic::AtomicPtr},
    thread,
};

use crossbeam::{
    atomic::AtomicCell,
    channel::{Receiver, unbounded},
    queue::SegQueue,
};

use super::{
    pool::{Config, State, ThreadPool},
    reactor::Reactor,
    task::{ID, IdManager, Task},
};

// 要满足在线程池中运行多个Future
/// 负责执行Future
struct Executor {
    /// 被执行Future队列
    ready_queue: Receiver<ID>,
    /// 线程池
    thread_pool: ThreadPool,
    /// 反应机
    reactor: Reactor,
    /// id管理者
    id_manager: IdManager,
    /// 对应下标,由唯一id分配
    ///
    /// 由于id唯一,且独立,可以安全的操控对应下标的Task
    future_list: Vec<Task>,
}

impl Executor {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded::<ID>();
        let thread_pool =
            ThreadPool::new(State::new(), Config::new(10, 500, 99999), 5);
        let future_list = Vec::new();
        let id_manager = IdManager::new();
        let reactor = Reactor::new(sender);
        let executor = Self {
            ready_queue: receiver.clone(),
            thread_pool,
            reactor,
            id_manager,
            future_list,
        };

        executor
    }
    /// 启动executor
    pub fn run(&mut self) {
        let ptr = &self.future_list;
        let ptr = ptr as *const Vec<Task> as *mut Vec<Task>;
        loop {
            let ptr = AtomicPtr::new(ptr);

            match self.ready_queue.recv() {
                Ok(id) => {
                    self.thread_pool.execute(move || {
                        // task.future.as_mut();
                        let list =
                            ptr.load(std::sync::atomic::Ordering::Relaxed);
                        list;
                    });
                }
                // 依照对应的 err ,做对应的处理
                _ => (),
            }
        }
    }

    /// #talk : task 是Task 还是Future呢
    pub fn on_block(&mut self, task: ()) {}
}

// executor 用线程池执行 future,
// 若 future pending ,则将 waker 放进reactor
//      注意,这个Future需要自行实现(将epoll中注册带有id的event,epoll会响应reactor,后reactor将id放入执行队列)
// 若 ready,则直接返回数据
//
// reactor 通过epoll监控,当future可被执行event发生后,将future的waker执行 通知executor去执行它
//
// waker用来告诉 executor, future可以被再次执行了
