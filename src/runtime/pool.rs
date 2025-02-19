use std::{
    sync::Arc,
    task::{Context, Waker},
    thread,
};

use crossbeam::channel::{Receiver, Sender, unbounded};
use fxhash::FxHashMap;

use super::{
    executor::{ID, NOW, TASK_SENDER},
    reactor::IdManager,
    task::Task,
};

/// 只管理线程,不管理任务
pub struct ThreadPool {
    pub task_sender: Sender<Task>,
    pub task_receiver: Receiver<Task>,
    pub woker: Vec<WokerThread>,
}
impl ThreadPool {
    /// `n`:创建n个工作线程
    pub fn new(n: usize, idea_dead_ms: u64) -> Self {
        let mut woker = Vec::with_capacity(n);
        let (task_sender, task_receiver) = unbounded();
        unsafe { TASK_SENDER = Some(task_sender.clone()) };
        for i in 0..n {
            woker.push(WokerThread::new(
                i as u64,
                task_receiver.clone(),
                idea_dead_ms,
            ));
        }
        Self {
            woker,
            task_sender,
            task_receiver,
        }
    }

    /// 添加工作线程
    pub fn add_woker(&mut self, idea_dead_ms: u64) {
        for (i, woker) in self.woker.iter().enumerate() {
            if woker.thread.is_finished() {
                self.woker[i] = WokerThread::new(
                    i as u64,
                    self.task_receiver.clone(),
                    idea_dead_ms,
                );
                break;
            }
        }
        self.woker.push(WokerThread::new(
            self.woker.len() as u64,
            self.task_receiver.clone(),
            idea_dead_ms,
        ));
    }
    pub fn add_task(&self, task: Task) {
        self.task_sender.send(task).unwrap();
    }
}
pub struct WokerThread {
    pub thread_id: ID,
    // pub task_sender: Sender<Task>,
    pub thread: thread::JoinHandle<()>,
    // /// 当前任务数量 (并非完全精准,只会在一批操作后更新任务数量)
    // pub amount: Arc<AtomicUsize>,
    // /// true:强制停止当前工作线程
    // stop:AtomicBool,
    /// 线程空闲 死亡时间
    /// 配置后,不可修改
    idle_dead_ms: u64,
}
impl WokerThread {
    pub fn new(
        id: ID,
        task_receiver: Receiver<Task>,
        idle_dead_ms: u64,
    ) -> Self {
        // let (sender, receiver) = unbounded();
        let builder =
            thread::Builder::new().name(format!("work_thread[{}]", id));
        // let amount = Arc::new(AtomicUsize::new(0));
        // let amount_ = amount.clone();
        let thread = builder
            .spawn(move || {
                let mut map = FxHashMap::<ID, Task>::default();
                // 事实上,id可以全局分配,在task创建时分配,但没法有效率的回收id
                let mut id_manager = IdManager::new();
                let task_receiver: crossbeam::channel::Receiver<Task> =
                    task_receiver;
                let (id_sender, id_receiver) = unbounded::<ID>();
                // let amount = amount_;
                let idle_dead_ms = idle_dead_ms.clone();
                // 当前空闲时间(now-time_anchor)
                let mut time_anchor = 0u128;
                loop {
                    // 接受外部的task
                    // 如果 A线程的receiver 会抢掉全部任务,
                    // 我们就需要限制每个线程每次最多接受的任务数量
                    let recv_task_max = 3;
                    let mut recv_count = 0;
                    while let Ok(mut task) = task_receiver.try_recv() {
                        let id = id_manager.get_id();
                        // id由线程内部管理,无论如何都需要改变它
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
                        recv_count += 1;
                        if recv_count == recv_task_max {
                            break;
                        }
                    }
                    // amount
                    //     .store(map.len(), std::sync::atomic::Ordering::SeqCst);
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
                    // 任务数量
                    let a = task_receiver.len() + map.len();
                    // amount.store(a, std::sync::atomic::Ordering::SeqCst);
                    // true:空闲
                    if a == 0 {
                        let now = NOW.elapsed().as_millis();
                        if time_anchor == 0 {
                            time_anchor = now;
                        }
                        if now - time_anchor >= idle_dead_ms.into() {
                            break;
                        }
                    } else {
                        time_anchor = 0;
                    }
                }
            })
            .unwrap();
        Self {
            thread_id: id,
            // task_sender: sender,
            thread,
            // amount,
            // stop: AtomicBool::new(false),
            idle_dead_ms,
        }
    }

    // pub fn add_task(&self, task: Task) {
    //     self.task_sender.send(task).unwrap();
    // }
}
