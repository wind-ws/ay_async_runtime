

用epoll（只支持linux）实现一个最小功能的**线程池**异步运行时，具体效果就是
1、实现一个TcpStream，具体api仿照std里的TcpStream，但是是异步版本
2、实现一个在main上的attribute proc macro，类似于tokio::main和tokio::test，使得可以将main或单元测试变成异步函数，并且不破坏rust analyzer对于函数内容的类型标注、鼠标悬停提示等。

要求，不使用任何形式的锁，包含std给的锁、parking_lot的锁或变相的自旋锁逻辑。

最新版rustc，可使用所有的非incomplete的unstable功能。
可使用std和所有跟线程同步、异步基础设施无关的crate。
代码中可有unsafe，但不可存在ub。
需要以下检查通过，并且没有任何错误/警告：
cargo miri test
cargo clippy
cargo fmt --check

# Note
* 不可使用锁,不可存在ub,不可使用 关于 线程同步,异步基础设施 的crate
* 
# Plan
0. 寻找源码,进行学习
1. 学习epoll的api
2. 学习关于Future的知识(Future,Waker,...)
3. 学习原子 无锁操作
4. 学习std的TcpStream的api
5. 学习attribute proc macro
6. 消除所有 warn , miri clippy fmt 都不会输出warn

# Blur Stage
* 包装epoll接口
* 先搭建 异步运行时 后处理 TcpStream
* 

# Stage (可能变动)
1. 包装epoll接口
2. 线程池
3. 线程池异步运行时
4. TcpStream异步化

# Project
实际项目名为: ay_async_runtime
宏名: ay ,例如:#[ay::main]
