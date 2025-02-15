//! 线程池 模块

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crossbeam::queue::SegQueue;

/// 线程池中被运行的单位(任务)
/// #check : 是否需要Pin
pub type Runnable = Box<dyn FnOnce() + Send + 'static>;
/// 工作线程返回值
pub type ThreadReturn = ();

/// 线程池
pub struct ThreadPool {
    /// 线程池状态
    state: State,
    /// 线程池配置
    config: Config,
    /// 任务队列
    queue: Arc<SegQueue<Runnable>>,
    /// 工作线程
    workers: Vec<thread::JoinHandle<ThreadReturn>>,
}

// #note: 以下所有函数并未被完善
impl ThreadPool {
    /// `size`:初始线程数量
    pub fn new(state: State, config: Config, size: usize) -> Self {
        let queue = Arc::new(SegQueue::<Runnable>::new());
        let mut workers: Vec<thread::JoinHandle<ThreadReturn>> =
            Vec::with_capacity(size);
        for _ in 0..size {
            // let thread_stop = Arc::clone(&state.thread_stop);
            let queue = Arc::clone(&queue);
            let worker = thread::spawn(move || {
                // 工作线程 打盹休息时常
                const GAP: u64 = 10;
                // 单位:ms
                let mut count = 0u64;
                // while !thread_stop.load(Ordering::SeqCst) {
                loop {
                    // 尝试获取一个任务并执行
                    if let Some(runnable) = queue.pop() {
                        count = 0;
                        runnable();
                    } else {
                        // 若没有任务，则等待一会
                        thread::sleep(Duration::from_millis(GAP));
                        count += GAP;
                        if count >= config.thread_dead_ms {
                            break;
                        }
                    }
                }
            });
            workers.push(worker);
        }
        Self {
            state,
            config,
            queue,
            workers,
        }
    }
    /// 为线程池中添加一个 任务
    /// 
    /// @return: None:添加成功, Some(f):添加失败
    pub fn execute<F>(&self, f: F)->Option<F>
    where
        F: FnOnce() + Send + 'static,
    {
        self.queue.push(Box::new(f));
        todo!()
    }
    // 停止线程池工作
    // #talk: 是否应该把任务队列的任务做完后停止?
    pub fn stop(&self) {}
}
impl Drop for ThreadPool {
    fn drop(&mut self) {
        // 确保没有其他任务,若存在,则需要等待其他任务执行完毕(设置等待时间上限)
        // 确保所有工作线程,处于空闲状态
        // 释放所有工作线程
        todo!()
    }
}

/// 线程池状态
struct State {
    // 空闲的线程数
    idle_count: usize,
    // 当前总线程数 (不可超过最大线程数)
    thread_count: usize,
    /// true: 拒绝接受所有任务
    /// #wait : 可能不需要,准备删除
    stop: AtomicBool,
    /// 停止所有工作线程(正在执行的不停止,执行完毕后停止)
    /// #wait : 可能不需要,准备删除
    thread_stop: Arc<AtomicBool>,
}
/// 线程池配置
struct Config {
    /// 最大线程数
    max_thread: usize,
    /// 线程空闲多少ms后 死亡
    thread_dead_ms: u64,
    /// 最大任务数(限制队列)
    max_queue: usize,
}

#[cfg(test)]
mod tests_thread_pool {

}
