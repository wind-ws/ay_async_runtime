//! 在 Future执行过程中无法推进需要等待时，需要将 Executor提供的 Waker注册在 Reactor上,
//! Reactor负责监听 Future是否Ready,
//! 如果Ready，则通过 Waker通知 Executor继续执行对应的 Future

use std::{io, os::fd::RawFd};


struct Reactor;




/// fd拥有者\
/// 用于创建和销毁epoll实例
struct FdOwner{
    fd:RawFd
}
impl FdOwner {
    pub fn new() -> io::Result<Self> {
        match crate::epoll::create() {
            Ok(fd) => Ok(FdOwner { fd }),
            Err(e) => Err(e),
        }
    }
}
impl Drop for FdOwner {
    fn drop(&mut self) {
        crate::epoll::close(self.fd).unwrap();
    }
}
/// fd使用者
struct FdUser {
    fd:FdOwner,
}

