use std::task::Wake;

use crossbeam::channel::Sender;

use super::executor::{ID, MyFuture};

/// #done
pub struct Task {
    pub id: ID,
    pub future: MyFuture,
}
impl Task {
    pub fn waker(&self, sender: Sender<ID>) -> TaskWaker {
        TaskWaker {
            id: self.id,
            sender,
        }
    }
}
/// #done
pub struct TaskWaker {
    pub id: ID,
    pub sender: Sender<ID>,
}
impl Wake for TaskWaker {
    fn wake(self: std::sync::Arc<Self>) {
        self.sender.send(self.id).unwrap();
    }
}
