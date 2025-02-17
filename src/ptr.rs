#![allow(unsafe_code)]


use core::fmt::{self, Formatter, Pointer};
use core::{
    cell::UnsafeCell, marker::PhantomData, mem::ManuallyDrop, num::NonZeroUsize, ptr::NonNull,
};

/// 用于 [`Ptr`]、[`PtrMut`] 和 [`OwningPtr`] 类型的参数，表示指定的指针是 对齐的
#[derive(Copy, Clone)]
pub struct Aligned;

/// 用于 [`Ptr`]、[`PtrMut`] 和 [`OwningPtr`] 类型的参数，表示指定的指针是 不对齐的
#[derive(Copy, Clone)]
pub struct Unaligned;

/// Trait that is only implemented for [`Aligned`] and [`Unaligned`] to work around the lack of ability
/// to have const generics of an enum.
/// 用于 标记和限定 类型只能是 Aligned或Unaligned\
/// 这种标记方式允许在编译时对指针的对齐属性进行静态检查
pub trait IsAligned: sealed::Sealed {}
impl IsAligned for Aligned {}
impl IsAligned for Unaligned {}

mod sealed {
    /// 用于限定内部trait, 不让外部类型实现那个trait
    pub trait Sealed {}
    impl Sealed for super::Aligned {}
    impl Sealed for super::Unaligned {}
}

/// A newtype around [`NonNull`] that only allows conversion to read-only borrows or pointers.
///
/// This type can be thought of as the `*const T` to [`NonNull<T>`]'s `*mut T`.
/// 
/// 用于封装不可变非空指针的类型\
/// 类型主要用于只读的指针操作，它等价于*const T\
/// 仅允许转换为 只读借用 或 指针
#[repr(transparent)]
pub struct ConstNonNull<T: ?Sized>(NonNull<T>);

impl<T: ?Sized> ConstNonNull<T> {
    /// 如果 `ptr` 是非空的 , 则创建一个 `ConstNonNull`
    /// 
    /// # Examples
    ///
    /// ```
    /// use bevy_ptr::ConstNonNull;
    ///
    /// let x = 0u32;
    /// let ptr = ConstNonNull::<u32>::new(&x as *const _).expect("ptr is null!");
    ///
    /// if let Some(ptr) = ConstNonNull::<u32>::new(std::ptr::null()) {
    ///     unreachable!();
    /// }
    /// ```
    pub fn new(ptr: *const T) -> Option<Self> {
        NonNull::new(ptr.cast_mut()).map(Self)
    }

    /// 不检查的创建一个 `ConstNonNull`.
    ///
    /// # Safety
    ///
    /// `ptr` 必须是非空的
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ptr::ConstNonNull;
    ///
    /// let x = 0u32;
    /// let ptr = unsafe { ConstNonNull::new_unchecked(&x as *const _) };
    /// ```
    ///
    /// 该函数的*不正确*用法：
    ///
    /// ```rust,no_run
    /// use bevy_ptr::ConstNonNull;
    ///
    /// // 永远不要这样做！这是未定义的行为。 ⚠️
    /// let ptr = unsafe { ConstNonNull::<u32>::new_unchecked(std::ptr::null()) };
    /// ```
    pub const unsafe fn new_unchecked(ptr: *const T) -> Self {
        // SAFETY: This function's safety invariants are identical to `NonNull::new_unchecked`
        // The caller must satisfy all of them.
        unsafe { Self(NonNull::new_unchecked(ptr.cast_mut())) }
    }

    /// 返回一个共享引用
    ///
    /// # Safety
    ///
    /// 调用此方法时，必须确保以下所有条件都成立：
    ///
    /// * 指针必须正确对齐
    ///
    /// * It must be "dereferenceable" in the sense defined in [the module documentation].\
    ///   指针在[模块文档]中定义的意义上必须是“可解引用”的
    ///
    /// * The pointer must point to an initialized instance of `T`.\
    ///   指针必须指向一个已初始化的 `T` 类型实例
    ///
    /// * You must enforce Rust's aliasing rules, since the returned lifetime `'a` is
    ///   arbitrarily chosen and does not necessarily reflect the actual lifetime of the data.
    ///   In particular, while this reference exists, the memory the pointer points to must
    ///   not get mutated (except inside `UnsafeCell`).\
    ///   你必须强制遵守Rust的别名规则，因为返回的生命周期 `'a` 是任意选择的，并不一定反映数据的实际生命周期。
    ///   特别是，在这个引用存在的期间，指针所指向的内存不应该被修改（除了在 `UnsafeCell` 内部）。
    ///
    /// This applies even if the result of this method is unused!
    /// (The part about being initialized is not yet fully decided, but until
    /// it is, the only safe approach is to ensure that they are indeed initialized.)\
    /// 即使此方法的返回结果未被使用，这些规则仍然适用！
    /// （关于是否要求初始化的部分尚未完全确定，但在这之前，唯一安全的方法是确保它们确实已被初始化。）
    ///
    /// # Examples
    ///
    /// ```
    /// use bevy_ptr::ConstNonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = ConstNonNull::new(&mut x as *mut _).expect("ptr is null!");
    ///
    /// let ref_x = unsafe { ptr.as_ref() };
    /// println!("{ref_x}");
    /// ```
    ///
    /// [the module documentation]: core::ptr#safety
    #[inline]
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        // SAFETY: This function's safety invariants are identical to `NonNull::as_ref`
        // The caller must satisfy all of them.
        unsafe { self.0.as_ref() }
    }
}

impl<T: ?Sized> From<NonNull<T>> for ConstNonNull<T> {
    fn from(value: NonNull<T>) -> ConstNonNull<T> {
        ConstNonNull(value)
    }
}

impl<'a, T: ?Sized> From<&'a T> for ConstNonNull<T> {
    fn from(value: &'a T) -> ConstNonNull<T> {
        ConstNonNull(NonNull::from(value))
    }
}

impl<'a, T: ?Sized> From<&'a mut T> for ConstNonNull<T> {
    fn from(value: &'a mut T) -> ConstNonNull<T> {
        ConstNonNull(NonNull::from(value))
    }
}

/// 类型擦除后的指针类型，用于在不知道具体类型的情况下处理指针
/// 
/// 类型擦除的不可变借用，指向在构造此类型时选择的未知类型。
///
/// 这种类型试图表现得像“借用”，这意味着:
/// - 它应被视为不可变的：在此指针存在期间，其指向的目标不得改变
/// - 它必须始终指向一个有效的值，无论被指向的类型是什么。
/// - 生命周期 `'a` 准确地表示了指针的有效时间
/// - 对于未知的被指向类型，指针必须有足够的对齐
/// 
/// 可以将此类型类比为 `&'a dyn Any`，但没有元数据，且能够指向不对应于Rust类型的数据
/// 
#[derive(Copy, Clone, Debug)]
#[repr(transparent)]
pub struct Ptr<'a, A: IsAligned = Aligned>(NonNull<u8>, PhantomData<(&'a u8, A)>);

/// 类型擦除的可变借用，指向在构造此类型时选择的未知类型
/// 
/// 这种类型试图表现得像“借用”，这意味着:
/// - 指针被视为独占(排他性)且可变的(可变性)。它不能被克隆，因为这会导致别名的可变性问题。
/// - 它必须始终指向一个有效的值，无论被指向的类型是什么。
/// - 生命周期 `'a` 准确地表示了指针的有效时间。
/// - 对于未知的被指向类型，指针必须有足够的对齐。
///
/// 可以将此类型类比为 `&'a mut dyn Any`，但没有元数据，且能够指向不对应于Rust类型的数据。
///
///  [`PtrMut`] 指针被视为独占的，这意味着在其生命周期内，它是唯一指向该数据的可变指针
#[derive(Debug)]
#[repr(transparent)]
pub struct PtrMut<'a, A: IsAligned = Aligned>(NonNull<u8>, PhantomData<(&'a mut u8, A)>);

/// [`OwningPtr`] 代表一个拥有所有权的指针, 它用于管理指针所指向数据的生命周期
/// 
/// 类似Box的类型擦除指针，指向在构造此类型时选择的未知类型
/// 
/// 在概念上，这表示对所指向数据的所有权，因此负责调用其 `Drop` 实现.
/// 但该指针不负责释放指向的内存，因为它可能指向 `Vec` 中的一个元素，或者指向函数中的局部变量等.
/// 
/// 这种类型试图表现得像“借用”，这意味着：
/// - 指针应被视为独占且可变的。它不能被克隆，因为这会导致别名的可变性问题，并且可能会引发在释放后使用的错误
/// - 它必须始终指向一个有效的值，无论被指向的类型是什么
/// - 生命周期 `'a` 准确地表示了指针的有效时间
/// - 对于未知的被指向类型，指针必须有足够的对齐
/// 
/// 可以将此类型类比为 `&'a mut ManuallyDrop<dyn Any>`，但没有元数据，且能够指向不对应于Rust类型的数据
/// 
#[derive(Debug)]
#[repr(transparent)]
pub struct OwningPtr<'a, A: IsAligned = Aligned>(NonNull<u8>, PhantomData<(&'a mut u8, A)>);

macro_rules! impl_ptr {
    ($ptr:ident) => {
        impl<'a> $ptr<'a, Aligned> {
            /// 删除此指针的对齐要求
            pub fn to_unaligned(self) -> $ptr<'a, Unaligned> {
                $ptr(self.0, PhantomData)
            }
        }

        impl<'a, A: IsAligned> From<$ptr<'a, A>> for NonNull<u8> {
            fn from(ptr: $ptr<'a, A>) -> Self {
                ptr.0
            }
        }

        impl<A: IsAligned> $ptr<'_, A> {
            /// 计算指针的偏移量
            /// 
            /// 由于指针已被类型擦除，因此没有可用的大小信息。提供的 `count` 参数是以字节为单位
            ///
            /// *See also: [`ptr::offset`][ptr_offset]*
            ///
            /// # Safety
            /// - 偏移量不能使现有的指针为null，或使其超出其分配范围
            /// - 如果类型参数 `A` 是 [`Aligned`]，则偏移不能使结果指针对被指向的类型而言变得不对齐
            /// - 结果指针所指向的值的生命周期必须超过此指针的生命周期
            ///
            /// [ptr_offset]: https://doc.rust-lang.org/std/primitive.pointer.html#method.offset
            #[inline]
            pub unsafe fn byte_offset(self, count: isize) -> Self {
                Self(
                    // SAFETY: The caller upholds safety for `offset` and ensures the result is not null.
                    unsafe { NonNull::new_unchecked(self.as_ptr().offset(count)) },
                    PhantomData,
                )
            }

            /// 计算相对于指针的偏移量 (convenience for `.offset(count as isize)`).
            /// 
            /// 由于指针已被类型擦除，因此没有可用的大小信息。提供的 `count` 参数是以字节为单位
            ///
            /// *See also: [`ptr::add`][ptr_add]*
            ///
            /// # Safety
            /// - 偏移量不能使现有的指针为null，或使其超出其分配范围
            /// - 如果类型参数 `A` 是 [`Aligned`]，则偏移不能使结果指针对被指向的类型而言变得不对齐
            /// - 结果指针所指向的值的生命周期必须超过此指针的生命周期
            ///
            /// [ptr_add]: https://doc.rust-lang.org/std/primitive.pointer.html#method.add
            #[inline]
            pub unsafe fn byte_add(self, count: usize) -> Self {
                Self(
                    // SAFETY: The caller upholds safety for `add` and ensures the result is not null.
                    unsafe { NonNull::new_unchecked(self.as_ptr().add(count)) },
                    PhantomData,
                )
            }
        }

        impl<A: IsAligned> Pointer for $ptr<'_, A> {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                Pointer::fmt(&self.0, f)
            }
        }
    };
}

impl_ptr!(Ptr);
impl_ptr!(PtrMut);
impl_ptr!(OwningPtr);

impl<'a, A: IsAligned> Ptr<'a, A> {
    /// 从原始指针创建一个新的实例
    ///
    /// # Safety
    /// - `inner` 必须指向一个有效的值，无论被指类型是什么
    /// - 如果类型参数 `A` 是 [`Aligned`]，那么 `inner` 必须对被指类型有足够的对齐
    /// - `inner` 必须具有正确的来源，以允许读取被指类型的数据
    /// - 生命周期 `'a` 必须受到限制，以确保在此 [`Ptr`] 有效期间，该指针始终有效，并且在此 [`Ptr`] 存在时，除了通过 [`UnsafeCell`]，不能对被指向的数据进行修改
    #[inline]
    pub unsafe fn new(inner: NonNull<u8>) -> Self {
        Self(inner, PhantomData)
    }

    /// 将此 [`Ptr`] 转换为 [`PtrMut`]
    ///
    /// # Safety
    /// 在第一个 [`PtrMut`] 被释放之前，不能为相同的 [`Ptr`] 创建另一个 [`PtrMut`]
    #[inline]
    pub unsafe fn assert_unique(self) -> PtrMut<'a, A> {
        PtrMut(self.0, PhantomData)
    }

    /// 将此 [`Ptr<T>`] 转换为具有相同生命周期的 `&T`
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`Ptr`].\
    ///   `T` 必须是这个 [`Ptr`] 擦除前所指类型
    /// - If the type parameter `A` is [`Unaligned`] then this pointer must be sufficiently aligned
    ///   for the pointee type `T`.\
    ///   如果类型参数 `A` 是 [`Unaligned`]，那么这个指针必须对被指类型 `T` 有足够的对齐
    #[inline]
    pub unsafe fn deref<T>(self) -> &'a T {
        let ptr = self.as_ptr().cast::<T>().debug_ensure_aligned();
        // SAFETY: The caller ensures the pointee is of type `T` and the pointer can be dereferenced.
        unsafe { &*ptr }
    }

    /// 取底层指针，并擦除关联的生命周期
    ///
    /// 如果可能的话，强烈建议使用 [`deref`](Self::deref) 代替此函数,因为它保留了生命周期。
    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn as_ptr(self) -> *mut u8 {
        self.0.as_ptr()
    }
}

impl<'a, T> From<&'a T> for Ptr<'a> {
    #[inline]
    fn from(val: &'a T) -> Self {
        // SAFETY: The returned pointer has the same lifetime as the passed reference.
        // Access is immutable.
        unsafe { Self::new(NonNull::from(val).cast()) }
    }
}

impl<'a, A: IsAligned> PtrMut<'a, A> {
    /// 从原始指针创建一个新的实例
    ///
    /// # Safety
    /// - `inner` 必须指向一个有效的值，不论被指类型是什么
    /// - 如果类型参数 `A` 是 [`Aligned`]，那么 `inner` 必须对被指类型有足够的对齐
    /// - `inner` 必须有正确的来源，以允许对被指类型进行读写操作
    /// - 生命周期 `'a` 必须受到限制，以确保在这个 [`PtrMut`] 有效期间，它保持有效，并且在 [`PtrMut`] 存在期间，不能有其他实体读取或修改被指向的值
    #[inline]
    pub unsafe fn new(inner: NonNull<u8>) -> Self {
        Self(inner, PhantomData)
    }

    /// 将 [`PtrMut`] 转换为 [`OwningPtr`]
    ///
    /// # Safety
    /// Must have right to drop or move out of [`PtrMut`].\
    /// 必须拥有释放或移动出 [`PtrMut`] 的权利
    #[inline]
    pub unsafe fn promote(self) -> OwningPtr<'a, A> {
        OwningPtr(self.0, PhantomData)
    }

    /// 将这个 [`PtrMut<T>`] 转换为一个具有相同生命周期的 `&mut T`
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`PtrMut`].\
    ///   `T` 必须是这个 [`PtrMut`] 擦除前所指类型
    /// - If the type parameter `A` is [`Unaligned`] then this pointer must be sufficiently aligned
    ///   for the pointee type `T`.\
    ///   如果类型参数 `A` 是 [`Unaligned`]，那么这个指针必须对被指类型 `T` 有足够的对齐
    #[inline]
    pub unsafe fn deref_mut<T>(self) -> &'a mut T {
        let ptr = self.as_ptr().cast::<T>().debug_ensure_aligned();
        // SAFETY: The caller ensures the pointee is of type `T` and the pointer can be dereferenced.
        unsafe { &mut *ptr }
    }

    /// 获取底层指针，擦除关联的生命周期
    ///
    /// 如果可能，强烈建议使用 [`deref_mut`](Self::deref_mut) 方法代替此函数，因为它保留了生命周期
    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr()
    }

    /// Gets a [`PtrMut`] from this with a smaller lifetime.\
    /// 从中获取一个具有较小生命周期的 [`PtrMut`]。
    #[inline]
    pub fn reborrow(&mut self) -> PtrMut<'_, A> {
        // SAFETY: the ptrmut we're borrowing from is assumed to be valid
        unsafe { PtrMut::new(self.0) }
    }

    /// 从该可变引用获取不可变引用
    #[inline]
    pub fn as_ref(&self) -> Ptr<'_, A> {
        // SAFETY: The `PtrMut` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { Ptr::new(self.0) }
    }
}

impl<'a, T> From<&'a mut T> for PtrMut<'a> {
    #[inline]
    fn from(val: &'a mut T) -> Self {
        // SAFETY: The returned pointer has the same lifetime as the passed reference.
        // The reference is mutable, and thus will not alias.
        unsafe { Self::new(NonNull::from(val).cast()) }
    }
}

impl<'a> OwningPtr<'a> {
    /// Consumes a value and creates an [`OwningPtr`] to it while ensuring a double drop does not happen.
    #[inline]
    pub fn make<T, F: FnOnce(OwningPtr<'_>) -> R, R>(val: T, f: F) -> R {
        let mut temp = ManuallyDrop::new(val);
        // SAFETY: The value behind the pointer will not get dropped or observed later,
        // so it's safe to promote it to an owning pointer.
        f(unsafe { PtrMut::from(&mut *temp).promote() })
    }
}

impl<'a, A: IsAligned> OwningPtr<'a, A> {
    /// 从原始指针创建一个新的实例
    ///
    /// # Safety
    /// - `inner` 必须指向一个有效的值，不论被指类型是什么
    /// - 如果类型参数 `A` 是 [`Aligned`]，那么 `inner` 必须对被指类型有足够的对齐
    /// - `inner` 必须有正确的来源，以允许对被指类型进行读写操作
    /// - 生命周期 `'a` 必须受到约束，以确保这个 [`OwningPtr`] 在其有效期间保持有效，并且在此期间没有其他实体可以读取或修改指向的值。
    #[inline]
    pub unsafe fn new(inner: NonNull<u8>) -> Self {
        Self(inner, PhantomData)
    }

    /// 消费这个 [`OwningPtr`] 以获取底层 `T` 类型数据的所有权
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`OwningPtr`].\
    ///   `T` 必须是这个 [`OwningPtr`] 擦除前所指类型
    /// - If the type parameter `A` is [`Unaligned`] then this pointer must be sufficiently aligned
    ///   for the pointee type `T`.\
    ///   如果类型参数 `A` 是 [`Unaligned`]，那么这个指针必须对被指类型 `T` 有足够的对齐
    #[inline]
    pub unsafe fn read<T>(self) -> T {
        let ptr = self.as_ptr().cast::<T>().debug_ensure_aligned();
        // SAFETY: The caller ensure the pointee is of type `T` and uphold safety for `read`.
        unsafe { ptr.read() }
    }

    /// 消费这个 [`OwningPtr`] 以释放底层 `T` 类型的数据
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`OwningPtr`].\
    ///   `T` 必须是这个 [`OwningPtr`] 擦除前所指类型
    /// - If the type parameter `A` is [`Unaligned`] then this pointer must be sufficiently aligned
    ///   for the pointee type `T`.\
    ///   如果类型参数 `A` 是 [`Unaligned`]，那么这个指针必须对被指类型 `T` 有足够的对齐
    #[inline]
    pub unsafe fn drop_as<T>(self) {
        let ptr = self.as_ptr().cast::<T>().debug_ensure_aligned();
        // SAFETY: The caller ensure the pointee is of type `T` and uphold safety for `drop_in_place`.
        unsafe {
            ptr.drop_in_place();
        }
    }

    /// 获取底层指针，擦除关联的生命周期
    ///
    /// 如果可能，强烈建议使用其他更类型安全的函数来代替此函数
    #[inline]
    #[allow(clippy::wrong_self_convention)]
    pub fn as_ptr(&self) -> *mut u8 {
        self.0.as_ptr()
    }

    /// Gets an immutable pointer from this owned pointer.\
    /// 从这个指针([`OwningPtr`])获取一个不可变指针([`Ptr`])
    #[inline]
    pub fn as_ref(&self) -> Ptr<'_, A> {
        // SAFETY: The `Owning` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { Ptr::new(self.0) }
    }

    /// Gets a mutable pointer from this owned pointer.\
    /// 从这个指针([`OwningPtr`])获取一个可变指针([`PtrMut`])
    #[inline]
    pub fn as_mut(&mut self) -> PtrMut<'_, A> {
        // SAFETY: The `Owning` type's guarantees about the validity of this pointer are a superset of `Ptr` s guarantees
        unsafe { PtrMut::new(self.0) }
    }
}

impl<'a> OwningPtr<'a, Unaligned> {
    /// 消费这个 [`OwningPtr`] 以获取类型为 `T` 的底层数据所有权
    ///
    /// # Safety
    /// - `T` must be the erased pointee type for this [`OwningPtr`].\
    ///   `T` 必须是这个 [`OwningPtr`] 擦除前所指类型
    pub unsafe fn read_unaligned<T>(self) -> T {
        let ptr = self.as_ptr().cast::<T>();
        // SAFETY: The caller ensure the pointee is of type `T` and uphold safety for `read_unaligned`.
        unsafe { ptr.read_unaligned() }
    }
}

/// 概念上相当于“&'a [T]”，但出于性能原因删除了长度信息
pub struct ThinSlicePtr<'a, T> {
    ptr: NonNull<T>,
    #[cfg(debug_assertions)]
    len: usize,
    _marker: PhantomData<&'a [T]>,
}

impl<'a, T> ThinSlicePtr<'a, T> {
    /// 对切片进行索引而不进行边界检查
    ///
    /// # Safety
    /// `index` 必须在边界范围内
    #[inline]
    pub unsafe fn get(self, index: usize) -> &'a T {
        #[cfg(debug_assertions)]
        debug_assert!(index < self.len);

        let ptr = self.ptr.as_ptr();
        // SAFETY: `index` is in-bounds so the resulting pointer is valid to dereference.
        unsafe { &*ptr.add(index) }
    }
}

impl<'a, T> Clone for ThinSlicePtr<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Copy for ThinSlicePtr<'a, T> {}

impl<'a, T> From<&'a [T]> for ThinSlicePtr<'a, T> {
    #[inline]
    fn from(slice: &'a [T]) -> Self {
        let ptr = slice.as_ptr().cast_mut();
        Self {
            // SAFETY: a reference can never be null
            ptr: unsafe { NonNull::new_unchecked(ptr.debug_ensure_aligned()) },
            #[cfg(debug_assertions)]
            len: slice.len(),
            _marker: PhantomData,
        }
    }
}

/// 创建具有指定对齐方式的悬空指针
/// See [`NonNull::dangling`].
pub fn dangling_with_align(align: NonZeroUsize) -> NonNull<u8> {
    debug_assert!(align.is_power_of_two(), "Alignment must be power of two.");
    // SAFETY: The pointer will not be null, since it was created
    // from the address of a `NonZeroUsize`.
    unsafe { NonNull::new_unchecked(align.get() as *mut u8) }
}

mod private {
    use core::cell::UnsafeCell;

    pub trait SealedUnsafeCell {}
    impl<'a, T> SealedUnsafeCell for &'a UnsafeCell<T> {}
}

/// 用于在 [`UnsafeCell`] 上提供辅助方法的扩展特征
pub trait UnsafeCellDeref<'a, T>: private::SealedUnsafeCell {
    /// # Safety
    /// - The returned value must be unique and not alias any mutable or immutable references to the contents of the [`UnsafeCell`].\
    ///   返回的值必须是唯一的，不能与 [`UnsafeCell`] 内容的任何可变或不可变引用别名
    /// - At all times, you must avoid data races. If multiple threads have access to the same [`UnsafeCell`], then any writes must have a proper happens-before relation to all other accesses or use atomics ([`UnsafeCell`] docs for reference).\
    ///   在任何时候，你都必须避免数据竞争。如果多个线程访问同一个 [`UnsafeCell`]，那么任何写操作都必须与所有其他访问建立适当的前因后果关系，或者使用原子操作
    unsafe fn deref_mut(self) -> &'a mut T;

    /// # Safety
    /// - For the lifetime `'a` of the returned value you must not construct a mutable reference to the contents of the [`UnsafeCell`].\
    ///   在返回值的生命周期 `'a` 内，你不能构造对 [`UnsafeCell`] 内容的可变引用
    /// - At all times, you must avoid data races. If multiple threads have access to the same [`UnsafeCell`], then any writes must have a proper happens-before relation to all other accesses or use atomics ([`UnsafeCell`] docs for reference).\
    ///   在任何时候，你都必须避免数据竞争。如果多个线程访问同一个 [`UnsafeCell`]，那么任何写操作都必须与所有其他访问建立适当的前因后果关系，或者使用原子操作
    unsafe fn deref(self) -> &'a T;

    /// Returns a copy of the contained value.
    ///
    /// # Safety
    /// - The [`UnsafeCell`] must not currently have a mutable reference to its content.\
    ///   [`UnsafeCell`] 当前不能对其内容有可变引用
    /// - At all times, you must avoid data races. If multiple threads have access to the same [`UnsafeCell`], then any writes must have a proper happens-before relation to all other accesses or use atomics ([`UnsafeCell`] docs for reference).\
    ///   在任何时候，你都必须避免数据竞争。如果多个线程访问同一个 [`UnsafeCell`]，那么任何写操作都必须与所有其他访问建立适当的前因后果关系，或者使用原子操作
    unsafe fn read(self) -> T
    where
        T: Copy;
}

impl<'a, T> UnsafeCellDeref<'a, T> for &'a UnsafeCell<T> {
    #[inline]
    unsafe fn deref_mut(self) -> &'a mut T {
        // SAFETY: The caller upholds the alias rules.
        unsafe { &mut *self.get() }
    }
    #[inline]
    unsafe fn deref(self) -> &'a T {
        // SAFETY: The caller upholds the alias rules.
        unsafe { &*self.get() }
    }

    #[inline]
    unsafe fn read(self) -> T
    where
        T: Copy,
    {
        // SAFETY: The caller upholds the alias rules.
        unsafe { self.get().read() }
    }
}

trait DebugEnsureAligned {
    fn debug_ensure_aligned(self) -> Self;
}

// Disable this for miri runs as it already checks if pointer to reference
// casts are properly aligned.
#[cfg(all(debug_assertions, not(miri)))]
impl<T: Sized> DebugEnsureAligned for *mut T {
    #[track_caller]
    fn debug_ensure_aligned(self) -> Self {
        let align = core::mem::align_of::<T>();
        // Implementation shamelessly borrowed from the currently unstable
        // ptr.is_aligned_to.
        //
        // Replace once https://github.com/rust-lang/rust/issues/96284 is stable.
        assert_eq!(
            self as usize & (align - 1),
            0,
            "pointer is not aligned. Address {:p} does not have alignment {} for type {}",
            self,
            align,
            core::any::type_name::<T>()
        );
        self
    }
}

#[cfg(any(not(debug_assertions), miri))]
impl<T: Sized> DebugEnsureAligned for *mut T {
    #[inline(always)]
    fn debug_ensure_aligned(self) -> Self {
        self
    }
}
