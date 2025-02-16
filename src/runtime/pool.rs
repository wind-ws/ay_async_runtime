//! 线程池 模块

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
    state: Arc<State>,
    /// 线程池配置
    config: Arc<Config>,
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
        let workers: Vec<thread::JoinHandle<ThreadReturn>> =
            Vec::with_capacity(size);
        if size > config.max_thread {
            panic!("超过最大线程数量限制")
        }
        let mut pool = Self {
            state: Arc::new(state),
            config: Arc::new(config),
            queue,
            workers,
        };
        for _ in 0..size {
            pool.create_work_thread();
        }
        pool
    }

    fn create_work_thread(&mut self) {
        let queue = Arc::clone(&self.queue);
        let config = Arc::clone(&self.config);
        let state = Arc::clone(&self.state);
        state.thread_count.fetch_add(1, Ordering::Relaxed);
        state.idle_count.fetch_add(1, Ordering::Relaxed);
        let worker = thread::spawn(move || {
            // 工作线程 打盹休息时常
            const GAP: u64 = 10;
            // 单位:ms
            let mut count = 0u64;
            let mut b_sub = true;
            let mut b_add = false; 
            // while !thread_stop.load(Ordering::SeqCst) {
            loop {
                // 尝试获取一个任务并执行
                if let Some(runnable) = queue.pop() {
                    count = 0;
                    b_add = true;
                    if b_sub {
                        b_sub = false;
                        state.idle_count.fetch_sub(1, Ordering::Acquire);
                    }
                    runnable();
                    // #[cfg(test)]
                    // print!(
                    //     ": {} , {}|",
                    //     state.thread_count.load(Ordering::SeqCst),
                    //     state.idle_count.load(Ordering::SeqCst)
                    // );
                } else {
                    b_sub = true;
                    if b_add {
                        b_add = false;
                        state.idle_count.fetch_add(1, Ordering::Acquire);
                    }
                    // 若没有任务，则等待一会
                    thread::sleep(Duration::from_millis(GAP));
                    count += GAP;
                    if count >= config.thread_dead_ms {
                        break;
                    }
                }
            }
            state.idle_count.fetch_sub(1, Ordering::Relaxed);
            state.thread_count.fetch_sub(1, Ordering::Relaxed);
        });
        self.workers.push(worker);
    }
    /// 为线程池中添加一个 任务
    ///
    /// @return: None:添加成功, Some(f):添加失败
    pub fn execute<F>(&mut self, f: F) -> Result<(), F>
    where
        F: FnOnce() + Send + 'static,
    {
        if self.config.max_queue == self.queue.len() {
            // 达到任务添加上限
            return Err(f);
        }
        self.queue.push(Box::new(f));

        let thread_count = self.state.thread_count.load(Ordering::Relaxed);
        // 可添加线程数量
        let a = self.config.max_thread - thread_count;
        // 检查是否需要添加线程
        if a > 0 {
            // 可添加线程
            let idle_count = self.state.idle_count.load(Ordering::Relaxed);
            if self.queue.len() > idle_count * 2 {
                let b = (self.queue.len() - idle_count * 2) / 2;
                for _ in 0..a.min(b) {
                    self.create_work_thread();
                }
            }
        }

        // 检查是否需要删除以结束的线程
        // 以结束的线程数量
        let a = self.workers.len()- thread_count;
        // #todo
        
        Ok(())
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
        // todo!()
    }
}

/// 线程池状态
pub struct State {
    // 空闲的线程数
    pub idle_count: AtomicUsize,
    // 当前总线程数 (不可超过最大线程数)
    pub thread_count: AtomicUsize,
    /// true: 拒绝接受所有任务
    /// #wait : 可能不需要,准备删除
    stop: AtomicBool,
    /// 停止所有工作线程(正在执行的不停止,执行完毕后停止)
    /// #wait : 可能不需要,准备删除
    thread_stop: Arc<AtomicBool>,
}
impl State {
    pub fn new() -> Self {
        Self {
            idle_count: AtomicUsize::new(0),
            thread_count: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            thread_stop: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// 线程池配置
pub struct Config {
    /// 最大线程数
    pub max_thread: usize,
    /// 线程空闲多少ms后 死亡
    pub thread_dead_ms: u64,
    /// 最大任务数(限制队列)
    pub max_queue: usize,
}
impl Config {
    pub fn new(
        max_thread: usize,
        thread_dead_ms: u64,
        max_queue: usize,
    ) -> Self {
        Self {
            max_thread,
            thread_dead_ms,
            max_queue,
        }
    }
}

#[cfg(test)]
mod tests_thread_pool {
    use super::*;
    #[test]
    fn test_run() {
        let mut pool =
            ThreadPool::new(State::new(), Config::new(10, 500, 99999), 5);
        for i in 0..10000 {
            match pool.execute(move|| {
                println!("{}", i);
            }) {
                Ok(_) => (),
                Err(_) => panic!("err:{}", i),
            }
        }
    }

}
