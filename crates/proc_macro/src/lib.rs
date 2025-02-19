use proc_macro::TokenStream as RawTokenStream;
use proc_macro2::TokenStream;
use syn::{
    Error, Ident, ItemFn, LitStr, Token, parse::Parse, parse_macro_input,
};

struct Args {
    pub worker_threads: usize,
}
impl Parse for Args {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let worker_threads_key: Ident = input.parse()?;
        if !worker_threads_key.eq("worker_threads") {
            return Err(Error::new_spanned(
                worker_threads_key,
                "key must be worker_threads",
            ));
        }
        let _eq: Token![=] = input.parse()?;
        let worker_threads_value: syn::LitInt = input.parse()?;

        let worker_threads = worker_threads_value.base10_parse::<usize>()?;
        Ok(Args { worker_threads })
    }
}

#[proc_macro_attribute]
pub fn ay_main(attrs: RawTokenStream, item: RawTokenStream) -> RawTokenStream {
    let attrs: TokenStream = attrs.into();
    let mut item: TokenStream = item.into();
    let attr: Args = match syn::parse2(attrs.clone()) {
        Ok(it) => it,
        Err(e) => {
            item.extend(e.into_compile_error());
            return item.into();
        }
    };
    
    let input_fn: ItemFn = match syn::parse2(item.clone()) {
        Ok(it) => it,
        Err(e) => {
            item.extend(e.into_compile_error());
            return item.into();
        }
    };

    let func_name = &input_fn.sig.ident; // 函数名
    let func_inputs = &input_fn.sig.inputs; // 函数输入参数
    let func_output = &input_fn.sig.output; // 函数返回参数
    let func_async = input_fn.sig.asyncness; // async声明
    let block = &input_fn.block;
    {
        // 函数名必须是main
        if !func_name.eq("main") {
            return Error::new_spanned(func_name, "function name must be main")
                .to_compile_error()
                .into();
        }
        // 必须是 async
        if func_async.is_none() {
            return Error::new_spanned(func_name, "function must be async")
                .to_compile_error()
                .into();
        }
    }
    let  worker_threads= attr.worker_threads;
    // 生成新的代码，将原函数修改为异步运行
    let expanded = quote::quote! {
         // 使用 Tokio 的运行时标记
        fn #func_name() {
            let mut executor = interview_rust_project::runtime::executor::Executor::new(#worker_threads, 500);
            let future = async { #block };
            let future = std::boxed::Box::pin(future);
            let task = interview_rust_project::runtime::task::Task {
                id: interview_rust_project::runtime::reactor::IdManager::INVALID_ID,
                future: future,
            };
            executor.add_task(task);
            executor.block();
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn ay_test(attrs: RawTokenStream, item: RawTokenStream) -> RawTokenStream {
    let attrs: TokenStream = attrs.into();
    let mut item: TokenStream = item.into();
    let attr: Args = match syn::parse2(attrs.clone()) {
        Ok(it) => it,
        Err(e) => {
            item.extend(e.into_compile_error());
            return item.into();
        }
    };
    
    let input_fn: ItemFn = match syn::parse2(item.clone()) {
        Ok(it) => it,
        Err(e) => {
            item.extend(e.into_compile_error());
            return item.into();
        }
    };

    let func_name = &input_fn.sig.ident; // 函数名
    let func_inputs = &input_fn.sig.inputs; // 函数输入参数
    let func_output = &input_fn.sig.output; // 函数返回参数
    let func_async = input_fn.sig.asyncness; // async声明
    let block = &input_fn.block;
    {
        // 必须是 async
        if func_async.is_none() {
            return Error::new_spanned(func_name, "function must be async")
                .to_compile_error()
                .into();
        }
    }
    let  worker_threads= attr.worker_threads;
    // 生成新的代码，将原函数修改为异步运行
    let expanded = quote::quote! {
         // 使用 Tokio 的运行时标记
        #[test]
        fn #func_name() {
            let mut executor = interview_rust_project::runtime::executor::Executor::new(#worker_threads, 500);
            let future = async { #block };
            let future = std::boxed::Box::pin(future);
            let task = interview_rust_project::runtime::task::Task {
                id: interview_rust_project::runtime::reactor::IdManager::INVALID_ID,
                future: future,
            };
            executor.add_task(task);
            executor.block();
        }
    };

    expanded.into()
}