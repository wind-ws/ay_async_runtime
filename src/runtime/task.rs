use std::{pin::Pin, sync::{atomic::AtomicUsize, mpsc::SyncSender}, task::Wake};

struct Task {
    id: ID,
    ready_queue: SyncSender<ID>,
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl Wake for Task {
    fn wake(self: std::sync::Arc<Self>) {
        todo!()
    }
}

type ID = usize;

/// 唯一id管理者(分配,回收)
struct IdManager{
    /// id计数
    count:AtomicUsize,
    /// bin的索引,用来解决 IdManager::01#err
    index:AtomicUsize,
    /// 回收的id
    /// 01#err: 无法保证,在多线程中,id被重复分配
    bin:Vec<usize>
}

