use crate::{PhantomData, SmallVec};
use std::mem::ManuallyDrop;
use std::ptr::NonNull;

pub struct DynSlot<'a, T: ?Sized + 'a> {
    vtable: NonNull<()>,
    data: SmallVec<[u8; 64]>,
    phantom: PhantomData<&'a T>,
}

#[macro_export]
macro_rules! dynslot_new {
    ($val:expr) => {{
        let mut val = $val;
        let slot = unsafe { DynSlot::new(&mut val as *mut _) };
        ::std::mem::forget(val);
        slot
    }};
}

#[macro_export]
macro_rules! dynslot_update {
    ($slot:expr, $val:expr) => {{
        let mut val = $val;
        unsafe {
            $slot.update(&mut val as *mut _);
        }
        ::std::mem::forget(val);
    }};
}

impl<'a, T: ?Sized + 'a> DynSlot<'a, T> {
    /// Use [dynslot_new]
    pub unsafe fn new(val: *mut T) -> Self {
        assert_eq!(
            std::mem::size_of::<*mut T>(),
            std::mem::size_of::<[usize; 2]>(),
            "not a trait object"
        );

        let mut slot = Self {
            vtable: NonNull::dangling(),
            data: SmallVec::new(),
            phantom: PhantomData,
        };

        slot.update_value(val);
        slot
    }

    /// First element in vtable must be destructor
    unsafe fn drop_current(&mut self) {
        if self.vtable != NonNull::dangling() {
            let destructor = *(self.vtable.as_ptr() as *mut fn(*mut u8));
            self.vtable = NonNull::dangling(); // dont call again even on panic
            destructor(self.data.as_mut_ptr());
        }
    }

    /// Value is copied out of ptr, ensure the value is forgotten
    unsafe fn update_value(&mut self, val: *mut T) {
        let raw_sz = std::mem::size_of_val(&*val);
        let aligned_sz = align_up(raw_sz, 2);
        let val = ManuallyDrop::new(val);

        // decompose into vtable and data
        let [data_addr, vtable_addr] = fatptr::decomp(&**val as *const T);

        // copy value
        self.data.resize(aligned_sz, 0);
        self.data
            .as_mut_ptr()
            .copy_from_nonoverlapping(data_addr as *const u8, raw_sz);

        // store vtable
        self.vtable = NonNull::new_unchecked(vtable_addr as *mut ());
    }

    /// Use [dynslot_update]
    pub unsafe fn update(&mut self, val: *mut T) {
        self.drop_current();
        self.update_value(val);
    }

    pub fn get(&self) -> &T {
        let components = [self.data.as_ptr() as usize, self.vtable.as_ptr() as usize];
        unsafe { &*fatptr::recomp(components) }
    }
}

impl<'a, T: ?Sized + 'a> Drop for DynSlot<'a, T> {
    fn drop(&mut self) {
        // safety: vtable is always valid unless dangling, which is checked
        unsafe {
            self.drop_current();
        }
    }
}

const fn align_up(val: usize, align: usize) -> usize {
    let align_bits = align.trailing_zeros();
    (val + align - 1) >> align_bits << align_bits
}

// thanks https://docs.rs/crate/dynstack/0.4.0/source/src/fatptr.rs
mod fatptr {
    pub unsafe fn decomp<T: ?Sized>(ptr: *const T) -> [usize; 2] {
        let ptr_ref: *const *const T = &ptr;
        let decomp_ref = ptr_ref as *const [usize; 2];
        *decomp_ref
    }

    pub unsafe fn recomp<T: ?Sized>(components: [usize; 2]) -> *mut T {
        let component_ref: *const [usize; 2] = &components;
        let ptr_ref = component_ref as *const *mut T;
        *ptr_ref
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::{Display, Formatter};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::*;

    #[test]
    fn dynslot() {
        let s = String::from("I am a string!!!");
        let mut slot: DynSlot<dyn Display> = dynslot_new!(s);
        assert_eq!(slot.get().to_string(), "I am a string!!!");

        struct DisplayThing1(u32);
        impl Display for DisplayThing1 {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "I am a display thing {:#x}", self.0)
            }
        }

        let thing = DisplayThing1(0x41424344);
        dynslot_update!(slot, thing);
        assert_eq!(slot.get().to_string(), "I am a display thing 0x41424344");

        let other = "wowee";
        dynslot_update!(slot, other);

        assert_eq!(slot.get().to_string(), "wowee");
    }

    #[test]
    fn large() {
        struct LargeThing([u64; 128]);
        impl Display for LargeThing {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self.0)
            }
        }

        let large = LargeThing([5; 128]);
        let small = 123;

        let mut slot: DynSlot<dyn Display> = dynslot_new!(small);
        assert!(!slot.data.spilled());

        let expected_display = format!("{}", large);
        dynslot_update!(slot, large);
        assert_eq!(slot.get().to_string(), expected_display);

        assert!(slot.data.spilled()); // surely must be on the heap now
    }

    #[test]
    fn leak_check() {
        let mut slot: DynSlot<dyn Display> = dynslot_new!("bah");
        for _ in 0..2000 {
            let string = "wawaweewa".to_string();
            dynslot_update!(slot, string);
        }
    }

    #[test]
    fn proper_destructor() {
        let mut drops = Arc::new(AtomicUsize::new(0));
        struct DropCheck(Arc<AtomicUsize>);

        impl Display for DropCheck {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "drop me!")
            }
        }

        impl Drop for DropCheck {
            fn drop(&mut self) {
                println!("dropped!");
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        {
            let mut slot: DynSlot<dyn Display> = dynslot_new!(DropCheck(drops.clone()));
            assert_eq!(drops.load(Ordering::Relaxed), 0); // not dropped yet

            // replace it
            dynslot_update!(slot, "haha");
            assert_eq!(drops.load(Ordering::Relaxed), 1); // boosh

            dynslot_update!(slot, DropCheck(drops.clone()));
            // slot falls out of scope, should drop it
        }

        assert_eq!(drops.load(Ordering::Relaxed), 2); // yeees
    }
}
