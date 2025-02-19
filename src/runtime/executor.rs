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

use std::{future, pin::Pin, process::Output, time::Instant};

use crossbeam::channel::Sender;
use lazy_static::lazy_static;

use super::{
    pool::ThreadPool,
    reactor::{IdManager, Reactor},
    task::Task,
};

pub type MyFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type ID = u64;

lazy_static! {
    pub static ref REACTOR: Reactor = Reactor::new();
    pub static ref NOW: Instant = Instant::now();
}
/// 无锁吹发落如叶,鬼魅寻法无仙答.
/// 梦中悟得解君愁,不得已全局变量.
pub static mut TASK_SENDER: Option<Sender<Task>> = None;

/// 分配任务 给其他线程
pub struct Executor {
    thread_pool: ThreadPool,
    /// 进入block状态
    block: bool,
}
impl Executor {
    pub fn new(n: usize, idea_dead_ms: u64) -> Self {
        Self {
            thread_pool: ThreadPool::new(n, idea_dead_ms),
            block: false,
        }
    }
    pub fn add_task(&self, task: Task) {
        self.thread_pool.add_task(task);
    }
    pub fn add_woker(&self, idea_dead_ms: u64) {
        if !self.block {
            self.add_woker(idea_dead_ms);
        }
    }
    /// 堵塞,直到所有任务执行完毕
    pub fn block(&mut self) {
        self.block = true;
        let mut n = 0;
        let len = self.thread_pool.woker.len();
        loop {
            if n == len {
                break;
            }
            let b = &self.thread_pool.woker[n].thread.is_finished();
            if *b {
                n = n + 1;
            }
        }
    }

    /// 堵塞一个future,并且获得返回值
    pub fn block_on(&self) {}

    pub fn spawn<F>(future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let future = Box::pin(future);
        let task = Task {
            id: IdManager::INVALID_ID,
            future: future,
        };
        #[allow(static_mut_refs)]
        unsafe { TASK_SENDER.as_ref().unwrap().send(task).unwrap() };
    }
}

mod test_executor3 {}
