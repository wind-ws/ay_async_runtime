//! 学习文档:
//! https://blog.csdn.net/Wufjsjjx/article/details/137143616?utm_medium=distribute.wap_relevant.none-task-blog-2~default~baidujs_baidulandingword~default-0-137143616-blog-122797672.237
//! https://zhuanlan.zhihu.com/p/187463036
//! https://www.cnblogs.com/imreW/p/17234004.html
//!
//! fd: 文件描述符 是一个用于标识和访问文件或输入/输出资源（如文件、网络套接字、管道等）的一种整数值。它是操作系统用来管理和追踪进程中打开的文件或其他 I/O 资源的方式
//!
//! libc::EINPROGRESS: 当你在非阻塞模式下执行 I/O 操作（如连接、读取或写入）时，如果操作无法立即完成，它将返回 EINPROGRESS
use std::io;

pub(crate) type EpollEvent = libc::epoll_event;

/// 创建一个epoll实例，获取其文件描述符
pub(crate) fn create() -> io::Result<i32> {
    // epoll_create从Linux 2.6.8内核版本开始，这个参数已经不再生效，可以传入任意值
    let res = unsafe { libc::epoll_create(1) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(res)
    }
}
/// #wait: 不一定需要它,需要时编写
fn create1() {}

/// 添加事件\
/// 将需要监听的文件描述符添加到epoll实例中，并指定需要监听的事件类型\
///
/// `epfd` : epoll实例的文件描述符\
/// `op` : 操作类型，可以取以下三个值之一：
/// 1. `EPOLL_CTL_ADD`: 将文件描述符fd添加到epoll实例中进行监听
/// 2. `EPOLL_CTL_MOD`: 修改已经在epoll实例中监听的文件描述符fd的监听事件
/// 3. `EPOLL_CTL_DEL`: 将文件描述符fd从epoll实例中移除，停止监听该文件描述符上的事件
///
/// `fd` : 需要被添加、修改或删除的文件描述符\
/// `event` : 指向epoll_event结构的指针，该结构用于指定需要监听的事件类型\
///
/// events:
/// `EPOLLONESHOT`: 单次事件触发后自动从 epoll 中移除文件描述符
/// `EPOLLIN`:  表示可读事件
/// `EPOLLOUT`: 表示可写事件
/// `EPOLLERR`: 表示错误事件
/// `EPOLLHUP`: 表示挂起事件
/// `EPOLLPRI`: 表示对应的文件描述符有紧急的数据可读（这里应该表示有带外数据到来）
/// `EPOLLET`:  将EPOLL设为边缘触发(Edge Triggered)模式，这是相对于水平触发(Level Triggered)来说的
pub(crate) fn ctl(
    epfd: i32,
    op: i32,
    fd: i32,
    event: &mut EpollEvent,
) -> io::Result<()> {
    let res = unsafe { libc::epoll_ctl(epfd, op, fd, event) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

///
/// `epfd`: epoll实例的文件描述符
/// events``: 指向epoll_event结构数组的指针，用于接收发生的事件信息
/// maxevents``: 指定最大返回事件数量，即events数组的大小
/// timeout``: 等待超时时间，单位为毫秒。传入-1表示无限等待，传入0表示立即返回，传入正整数表示等待超时时间
pub(crate) fn wait(
    epfd: i32,
    events: &mut [EpollEvent],
    maxevents: i32,
    timeout: i32,
) -> io::Result<i32> {
    let res = unsafe {
        libc::epoll_wait(epfd, events.as_mut_ptr(), maxevents, timeout)
    };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(res)
    }
}

/// 用于关闭一个文件描述符\
/// 当你不再需要通过某个文件描述符进行 I/O 操作时，调用 close 可以释放该描述符占用的内核资源\
pub(crate) fn close(fd: i32) -> io::Result<()> {
    let res = unsafe { libc::close(fd) };
    if res < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
