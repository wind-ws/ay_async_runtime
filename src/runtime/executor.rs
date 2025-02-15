//! 用户构造出的 Future最终需要提交到 Executor中执行

// 要满足在线程池中运行多个Future
struct Executor;


struct Waker;

struct AtmoicWaker;

