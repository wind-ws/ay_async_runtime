use std::{
    marker::PhantomPinned,
    pin::Pin,
    ptr::NonNull,
    sync::{atomic::AtomicUsize, mpsc::SyncSender},
    task::Wake,
};

use crossbeam::{channel::Sender, queue::SegQueue};

pub struct Task<Output = ()> {
    pub id: ID,
    pub future: Pin<Box<dyn Future<Output = Output> + Send>>,

    // _marker: PhantomPinned,
}
unsafe impl Send for Task {}
unsafe impl Sync for Task {}

impl<Output> Task<Output> {
    pub fn new(
        id: ID,
        future: Pin<Box<dyn Future<Output = Output> + Send>>,
    ) -> Self {
        Self {
            id,
            future,
            // _marker: PhantomPinned,
        }
    }
}

pub struct TaskWaker {
    pub id: ID,
    pub queue_sender: Sender<ID>,
}
unsafe impl Send for TaskWaker {}
unsafe impl Sync for TaskWaker {}

impl Wake for TaskWaker {
    fn wake(self: std::sync::Arc<Self>) {
        self.queue_sender
            .send(self.id)
            .expect("panic: Wake for Task");
    }
}


pub type ID = usize;

/// 唯一id管理者(分配,回收)
pub struct IdManager {
    /// id计数
    count: AtomicUsize,
    /// 回收的id
    ///
    /// #try: 当queue的长度为1时,表示以没有剩余的id,需取出最后的id,并且给queue分配 最后的id+1 的id
    bin: SegQueue<ID>,
}

impl IdManager {
    pub fn new() -> Self {
        Self {
            count: AtomicUsize::new(0),
            bin: SegQueue::new(),
        }
    }
}

/// id拥有者\
/// 由id管理者分配,当id拥有者drop后,会自动将id回收到id管理者bin中
pub struct IdOwner {
    id: ID,
    /// #wait: 也许一个是一个弱引用
    ptr: NonNull<SegQueue<ID>>,
}
impl IdOwner {
    pub fn id(&self) -> ID {
        self.id
    }
}
impl Drop for IdOwner {
    fn drop(&mut self) {
        todo!()
    }
}
