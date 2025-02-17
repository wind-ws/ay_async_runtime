//! 用户构造出的 Future最终需要提交到 Executor中执行

use std::{
    cell::UnsafeCell,
    pin::Pin,
    ptr::NonNull,
    sync::{Arc, atomic::AtomicPtr},
    task::{Context, Waker},
    thread,
};

use crossbeam::{
    atomic::AtomicCell,
    channel::{Receiver, Sender, unbounded},
    queue::SegQueue,
};

use super::{
    pool::{Config, State, ThreadPool},
    reactor::Reactor,
    task::{ID, IdManager, Task, TaskWaker},
};
use crate::ptr::{OwningPtr, PtrMut};

// 要满足在线程池中运行多个Future
/// 负责执行Future
struct Executor {
    /// 被执行Future队列
    queue_receiver: Receiver<ID>,

    queue_sender: Sender<ID>,

    /// 线程池
    thread_pool: ThreadPool,
    /// 反应机
    reactor: Reactor,
    /// id管理者
    id_manager: IdManager,
    /// 对应下标,由唯一id分配
    ///
    /// 由于id唯一,且独立,可以安全的操控对应下标的Task
    ///
    future_list: Vec<Pin<Box<Task>>>,

    waker_list: Vec<TaskWaker>,
}

impl Executor {
    pub fn new() -> Self {
        let (sender, receiver) = unbounded::<ID>();
        let thread_pool =
            ThreadPool::new(State::new(), Config::new(10, 500, 99999), 5);
        let future_list = Vec::new();
        let waker_list = Vec::new();
        let id_manager = IdManager::new();
        let reactor = Reactor::new(sender.clone());
        let executor = Self {
            queue_receiver: receiver.clone(),
            queue_sender: sender,
            thread_pool,
            reactor,
            id_manager,
            future_list,
            waker_list,
        };

        executor
    }

    /// 启动executor
    pub fn run(&mut self) {
        loop {
            match self.queue_receiver.recv() {
                Ok(id) => {
                    let ptr = self.future_list.get_mut(id).unwrap();
                    // let ptr = ptr as *mut Pin<Box<Task>>;

                    // let ptr = AtomicPtr::new(ptr);
                    let res = self.thread_pool.execute(move || {
                        //.
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

#[cfg(test)]
mod tests_executor {
    use std::{
        cell::UnsafeCell,
        collections::HashMap,
        future,
        ops::DerefMut,
        pin::{pin, Pin},
        sync::atomic::AtomicPtr,
        task::{Context, Waker},
        thread,
    };

    use crossbeam::epoch::{self, Atomic};

    use crate::runtime::task::Task;

    #[test]
    fn test() {
        let mut list: Vec<Task> = Vec::new();
        let mut map = HashMap::<usize, usize>::new();
        for i in 0..100 {
            let future = async move {
                println!("{}", i);
            };
            let task = Task::new(i, Box::pin(future));
            // let task = Box::pin(task);
            // map.insert(i, &task as *const Task  as usize);
            list.push(task);
            let ptr = list.get_mut(i).unwrap().future.as_mut();
            // let ptr = ptr as *const u8;
            // let ptr = unsafe { ptr.as_mut().unwrap().deref_mut() };

            // let ptr = AtomicPtr::new(ptr);
            // let task = ptr.load(std::sync::atomic::Ordering::Relaxed);
            // let task = unsafe { &mut *task };
            // println!("{}", task.id);
        }
    }
    #[test]
    fn test2() {
        // let future = async move {
        //     println!("abc{}", 0);
        // };
        // let mut future = Box::new(future);
        // let mut future = Atomic::new(pin!(future));
        // thread::spawn(move || {
        //     let epoch = epoch::pin();
        //     let mut cx = Context::from_waker(Waker::noop());
        //     future.load(std::sync::atomic::Ordering::Relaxed, &epoch);

        //     // unsafe {
        //     //     let a =(ptr.as_mut().unwrap());
        //     //     let a =Pin::new(a);
        //     //     a.poll(cx);
        //     // }
            
        // });
    }
}
