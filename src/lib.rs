use std::os::raw::c_void;
use std::ptr::{self, *};
use std::sync::{LazyLock, Mutex};

use libffi::raw::ffi_get_struct_offsets;
use libffi::{
    low::prep_cif,
    raw::{
        FFI_TYPE_STRUCT, ffi_abi_FFI_DEFAULT_ABI, ffi_call, ffi_cif, ffi_type, ffi_type_sint16,
        ffi_type_sint32,
    },
};
use libloading::Library;

type FnPtr = unsafe extern "C" fn();

static LIB: LazyLock<Mutex<Library>> = LazyLock::new(|| {
    Mutex::new(unsafe {
        Library::new("libs/libTESTLIB.dylib").expect("Couldn't find libTESTLIB.dylib")
    })
});

// Struct Elements, Struct Type
struct StructType(Vec<*mut ffi_type>, ffi_type);
unsafe impl Send for StructType {}
unsafe impl Sync for StructType {}

/*
    0 - Offset vector
    1 - Offset index into argument
    2 - Raw argument data
    3 - Offset index into return
    4 - Raw return data
*/
struct TargetType(Vec<usize>, usize, Vec<u8>, usize, Vec<u8>);
unsafe impl Send for TargetType {}
unsafe impl Sync for TargetType {}

static EXAMPLE_TYPE: LazyLock<Mutex<StructType>> = LazyLock::new(|| {
    Mutex::new({
        let mut struct_elements = vec![
            ptr::addr_of_mut!(ffi_type_sint32),
            ptr::addr_of_mut!(ffi_type_sint16),
            ptr::addr_of_mut!(ffi_type_sint32),
            ptr::addr_of_mut!(ffi_type_sint16),
            ptr::null_mut(),
        ];

        let struct_type = ffi_type {
            type_: FFI_TYPE_STRUCT,
            elements: struct_elements.as_mut_ptr(),
            ..Default::default()
        };

        StructType(struct_elements, struct_type)
    })
});

static TARGET: LazyLock<Mutex<TargetType>> =
    LazyLock::new(|| Mutex::new(TargetType(vec![], 0, vec![], 0, vec![])));

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_Init() {
    LazyLock::force(&LIB);
    LazyLock::force(&EXAMPLE_TYPE);
    println!("Success loading library and initializing struct type");
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_SetTarget() {
    let mut struct_type = EXAMPLE_TYPE.lock().unwrap();
    let mut target = TARGET.lock().unwrap();
    target.1 = 0;
    target.3 = 0;
    target.0.resize(struct_type.0.len() - 1, 0);

    unsafe {
        let _ = ffi_get_struct_offsets(
            ffi_abi_FFI_DEFAULT_ABI,
            ptr::addr_of_mut!(struct_type.1),
            target.0.as_mut_ptr(),
        );
    }
    // Set after get_struct_offsets
    let size = struct_type.1.size;

    println!("Offsets: {:?} Struct Size: {}", target.0, size);
    target.2.resize(size, 0);
    target.4.resize(size, 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_WriteU2(x: u16) {
    let mut target = TARGET.lock().unwrap();
    unsafe {
        let base = target.2.as_mut_ptr();
        write_unaligned(base.add(target.0[target.1]) as *mut u16, x);
        target.1 += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_WriteU4(x: u32) {
    let mut target = TARGET.lock().unwrap();
    unsafe {
        let base = target.2.as_mut_ptr();
        write_unaligned(base.add(target.0[target.1]) as *mut u32, x);
        target.1 += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_ReadU2() -> u16 {
    let mut target = TARGET.lock().unwrap();
    unsafe {
        let base = target.4.as_mut_ptr();
        let res: u16 = read_unaligned(base.add(target.0[target.3]) as *mut u16);
        target.3 += 1;
        res
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_ReadU4() -> u32 {
    let mut target = TARGET.lock().unwrap();
    unsafe {
        let base = target.4.as_mut_ptr();
        let res: u32 = read_unaligned(base.add(target.0[target.3]) as *mut u32);
        target.3 += 1;
        res
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_Call() {
    let mut struct_type = EXAMPLE_TYPE.lock().unwrap();
    let mut target = TARGET.lock().unwrap();
    let struct_data = target.2.as_mut_ptr();

    let mut arg_types: [*mut ffi_type; 1] = [&raw mut struct_type.1];
    unsafe {
        let lib = LIB.lock().unwrap();
        let fn_addr = lib.get::<FnPtr>(b"fn_b").unwrap();
        let fn_addr = Some(*fn_addr);

        let mut cif = std::mem::zeroed();
        prep_cif(
            &mut cif as *mut ffi_cif,
            ffi_abi_FFI_DEFAULT_ABI,
            1,
            &raw mut struct_type.1 as *mut ffi_type,
            arg_types.as_mut_ptr(),
        )
        .unwrap();

        let mut arg_values: [*mut c_void; 1] = [struct_data as *mut c_void];

        ffi_call(
            &mut cif as *mut ffi_cif,
            fn_addr,
            target.4.as_mut_ptr() as *mut c_void,
            arg_values.as_mut_ptr(),
        );
    }
}
