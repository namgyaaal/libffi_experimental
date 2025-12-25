use core::{num, slice};
use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::mem::zeroed;
use std::os::raw::c_void;
use std::ptr::{self, *};
use std::sync::{LazyLock, Mutex, MutexGuard};

use libffi::low::type_tag::STRUCT;
use libffi::raw::{
    ffi_arg, ffi_get_struct_offsets, ffi_status, ffi_status_FFI_OK, ffi_type_double,
    ffi_type_float, ffi_type_pointer, ffi_type_sint8, ffi_type_sint64, ffi_type_uint8,
    ffi_type_uint16, ffi_type_uint32, ffi_type_uint64, ffi_type_void,
};
use libffi::{
    low::prep_cif,
    raw::{
        FFI_TYPE_STRUCT, ffi_abi_FFI_DEFAULT_ABI, ffi_call, ffi_cif, ffi_type, ffi_type_sint16,
        ffi_type_sint32,
    },
};
use libloading::Library;

type FnPtr = unsafe extern "C" fn();

#[derive(Debug, Copy, Clone)]
enum Type {
    Value(*mut ffi_type),
    Struct(usize),
}
unsafe impl Send for Type {}
unsafe impl Sync for Type {}

struct StructType {
    type_: ffi_type,
    // Pointed to in type definition
    elements: Vec<*mut ffi_type>,
    offsets: Vec<usize>,
    // Kept for easy iteration
    children: Vec<Type>,
}
unsafe impl Send for StructType {}
unsafe impl Sync for StructType {}

struct Function {
    args: Vec<Type>,
    ret: Type,
    fn_: Option<FnPtr>,
}
unsafe impl Send for Function {}
unsafe impl Sync for Function {}

struct Target {
    arg_locations: Vec<usize>,
    write_idx: usize,
    write_buf: Vec<u8>,
    write_locations: Vec<usize>,
    read_idx: usize,
    read_buf: Vec<u8>,
    read_locations: Vec<usize>,
    fn_name: String,
}
unsafe impl Send for Target {}
unsafe impl Sync for Target {}

static LIB: LazyLock<Mutex<Library>> = LazyLock::new(|| {
    Mutex::new(unsafe {
        Library::new("libs/libTESTLIB.dylib").expect("Couldn't find libTESTLIB.dylib")
    })
});

static TYPE_TABLE: LazyLock<Mutex<Vec<Type>>> = LazyLock::new(|| {
    Mutex::new({
        vec![
            Type::Value(ptr::addr_of_mut!(ffi_type_void)),
            Type::Value(ptr::addr_of_mut!(ffi_type_uint8)),
            Type::Value(ptr::addr_of_mut!(ffi_type_uint16)),
            Type::Value(ptr::addr_of_mut!(ffi_type_uint32)),
            Type::Value(ptr::addr_of_mut!(ffi_type_uint64)),
            Type::Value(ptr::addr_of_mut!(ffi_type_sint8)),
            Type::Value(ptr::addr_of_mut!(ffi_type_sint16)),
            Type::Value(ptr::addr_of_mut!(ffi_type_sint32)),
            Type::Value(ptr::addr_of_mut!(ffi_type_sint64)),
            Type::Value(ptr::addr_of_mut!(ffi_type_float)),
            Type::Value(ptr::addr_of_mut!(ffi_type_double)),
            Type::Value(ptr::addr_of_mut!(ffi_type_pointer)),
        ]
    })
});
static STRUCT_TABLE: LazyLock<Mutex<Vec<Box<StructType>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

static FUNCTION_MAP: LazyLock<Mutex<HashMap<String, Function>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static TARGET: LazyLock<Mutex<Option<Target>>> = LazyLock::new(|| Mutex::new(None));

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_Init() {
    // Relevant for anything in the future
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_BuildStruct(ptr: *const u32, size: u32) -> i32 {
    let mut type_table = TYPE_TABLE.lock().unwrap();
    let mut struct_table = STRUCT_TABLE.lock().unwrap();

    let slice: &[u32] = unsafe { slice::from_raw_parts(ptr, size as usize) };
    let children: Vec<Type> = slice.iter().map(|x| type_table[*x as usize]).collect();
    let num_children = children.len();

    let mut struct_elements: Vec<*mut ffi_type> = children
        .iter()
        .map(|type_| match type_ {
            Type::Value(ptr) => *ptr,
            Type::Struct(idx) => std::ptr::addr_of_mut!(struct_table[*idx].type_),
        })
        .collect();
    struct_elements.push(ptr::null_mut());
    /*
        Type and offsets need to be generated after moving it into a place in memory it won't change from.
    */
    let mut struct_type = Box::new(StructType {
        type_: ffi_type::default(),
        elements: struct_elements,
        offsets: Vec::with_capacity(num_children),
        children: children,
    });
    struct_type.offsets.resize(num_children, 0);

    struct_type.type_ = ffi_type {
        type_: FFI_TYPE_STRUCT,
        elements: struct_type.elements.as_mut_ptr(),
        ..Default::default()
    };
    unsafe {
        let status = ffi_get_struct_offsets(
            ffi_abi_FFI_DEFAULT_ABI,
            ptr::addr_of_mut!(struct_type.type_),
            struct_type.offsets.as_mut_ptr(),
        );

        if status != ffi_status_FFI_OK {
            panic!("Internal error generating offsets for struct during generation");
        }
    };

    let s_idx = struct_table.len();
    let t_idx = type_table.len();
    struct_table.push(struct_type);
    type_table.push(Type::Struct(s_idx));
    t_idx as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_BuildFunction(
    name: *const c_char,
    arg_ptr: *const u32,
    arg_cnt: u32,
    ret_idx: u32,
) -> bool {
    let lib = LIB.lock().unwrap();
    let type_table = TYPE_TABLE.lock().unwrap();
    let mut function_map = FUNCTION_MAP.lock().unwrap();

    let name: String = unsafe { CStr::from_ptr(name).to_string_lossy().into_owned() };
    let fn_addr = unsafe { lib.get::<FnPtr>(name.clone()).unwrap() };
    let fn_addr = Some(*fn_addr);

    let slice: &[u32] = unsafe { slice::from_raw_parts(arg_ptr, arg_cnt as usize) };
    let args: Vec<Type> = slice.iter().map(|x| type_table[*x as usize]).collect();
    function_map.insert(
        name,
        Function {
            args: args,
            ret: type_table[ret_idx as usize],
            fn_: fn_addr,
        },
    );
    return true;
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_SetTarget(name: *const c_char) -> bool {
    let type_table = TYPE_TABLE.lock().unwrap();
    let struct_table = STRUCT_TABLE.lock().unwrap();
    let function_map = FUNCTION_MAP.lock().unwrap();
    let mut target = TARGET.lock().unwrap();

    let name: String = unsafe { CStr::from_ptr(name).to_string_lossy().into_owned() };
    let Some(fn_) = function_map.get(&name) else {
        return false;
    };
    let mut arg_locations: Vec<usize> = Vec::new();
    let mut arg_buf: Vec<u8> = Vec::new();
    let mut write_locations: Vec<usize> = Vec::new();

    for arg in &fn_.args {
        let old_len = arg_buf.len();
        let offsets = get_deep_struct_offsets(&type_table, &struct_table, *arg);

        if let Some(vec) = offsets {
            write_locations.extend_from_slice(&vec);
        } else {
            write_locations.push(old_len);
        }
        arg_locations.push(old_len);

        let type_size = match *arg {
            Type::Value(x) => unsafe { x.as_ref().unwrap().size },
            Type::Struct(idx) => struct_table[idx].type_.size,
        };
        // Safe enough with 8-multiple of padding per argument.
        arg_buf.resize(old_len + type_size.next_multiple_of(8), 0);
    }

    let mut ret_buf: Vec<u8> = Vec::new();
    let type_size = match *&fn_.ret {
        Type::Value(x) => unsafe { x.as_ref().unwrap().size },
        Type::Struct(idx) => struct_table[idx].type_.size,
    };
    ret_buf.resize(type_size, 0);

    let read_locations: Vec<usize> = {
        if let Some(vec) = get_deep_struct_offsets(&type_table, &struct_table, fn_.ret) {
            vec
        } else {
            vec![0]
        }
    };
    *target = Some(Target {
        arg_locations: arg_locations,
        write_idx: 0,
        write_buf: arg_buf,
        write_locations: write_locations,
        read_idx: 0,
        read_buf: ret_buf,
        read_locations: read_locations,
        fn_name: name,
    });
    return true;
}

#[unsafe(no_mangle)]
pub extern "C" fn FFIW_Call() {
    let mut struct_table = STRUCT_TABLE.lock().unwrap();

    let mut guard = TARGET.lock().unwrap();
    let mut function_map = FUNCTION_MAP.lock().unwrap();

    let target = guard
        .as_mut()
        .expect("Call SetTarget and write parameters before calling");

    let fn_ = function_map
        .get_mut(&target.fn_name)
        .expect("Internal Error in FFIW_Call loading function, this shouldn't happen");

    unsafe {
        let mut cif = std::mem::zeroed();

        let mut arg_types: Vec<*mut ffi_type> = fn_
            .args
            .iter()
            .map(|t| match *t {
                Type::Value(ptr) => ptr,
                Type::Struct(idx) => std::ptr::addr_of_mut!(struct_table[idx].type_),
            })
            .collect();

        let ret_type = match fn_.ret {
            Type::Value(ptr) => ptr,
            Type::Struct(idx) => std::ptr::addr_of_mut!(struct_table[idx].type_),
        };

        prep_cif(
            &mut cif as *mut ffi_cif,
            ffi_abi_FFI_DEFAULT_ABI,
            arg_types.len(),
            ret_type,
            arg_types.as_mut_ptr(),
        )
        .expect("Error calling prep_cif when calling function");

        let mut arg_values: Vec<*mut c_void> = Vec::new();
        // We have a contiguous buffer storing all the arguments in specific locations
        // Write them onto this vector.
        for loc in &target.arg_locations {
            let ptr = target.write_buf.as_mut_ptr().add(*loc);
            arg_values.push(ptr as *mut c_void);
        }
        ffi_call(
            &mut cif as *mut ffi_cif,
            fn_.fn_,
            target.read_buf.as_mut_ptr() as *mut c_void,
            arg_values.as_mut_ptr(),
        );
    }
}

fn write_generic<T: Copy>(x: T) {
    let mut guard = TARGET.lock().unwrap();
    let target = guard.as_mut().expect("Call SetTarget before writing");

    unsafe {
        let base = target.write_buf.as_mut_ptr();
        let location = target.write_locations[target.write_idx];

        write_unaligned(base.add(location) as *mut T, x);
        target.write_idx += 1;
    }
}

fn read_generic<T: Copy>() -> T {
    let mut guard = TARGET.lock().unwrap();
    let target = guard.as_mut().expect("Call SetTarget before writing");

    unsafe {
        let base = target.read_buf.as_mut_ptr();
        let location = target.read_locations[target.read_idx];

        let x: T = read_unaligned(base.add(location) as *mut T);
        target.read_idx += 1;
        x
    }
}

macro_rules! ffiw_write {
    ($w_name:ident, $r_name:ident, $ty:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $w_name(x: $ty) {
            write_generic::<$ty>(x);
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $r_name() -> $ty {
            read_generic::<$ty>()
        }
    };
}

ffiw_write!(FFIW_WriteU1, FFIW_ReadU1, u8);
ffiw_write!(FFIW_WriteU2, FFIW_ReadU2, u16);
ffiw_write!(FFIW_WriteU4, FFIW_ReadU4, u32);
ffiw_write!(FFIW_WriteU8, FFIW_ReadU8, u64);

ffiw_write!(FFIW_WriteI1, FFIW_ReadI1, i8);
ffiw_write!(FFIW_WriteI2, FFIW_ReadI2, i16);
ffiw_write!(FFIW_WriteI4, FFIW_ReadI4, i32);
ffiw_write!(FFIW_WriteI8, FFIW_ReadI8, i64);

ffiw_write!(FFIW_WriteF4, FFIW_ReadF4, f32);
ffiw_write!(FFIW_WriteF8, FFIW_ReadF8, f64);

ffiw_write!(FFIW_WritePtr, FFIW_ReadPtr, *mut c_void);

fn get_deep_struct_offsets(
    type_table: &MutexGuard<Vec<Type>>,
    struct_table: &MutexGuard<Vec<Box<StructType>>>,
    type_: Type,
) -> Option<Vec<usize>> {
    let Type::Struct(idx) = type_ else {
        return None;
    };

    let struct_type = &struct_table[idx];
    let mut offsets = struct_type.offsets.clone();

    let mut child_collection = Vec::new();
    for (i, &child) in struct_type.children.iter().enumerate() {
        let Some(mut child_offsets) = get_deep_struct_offsets(type_table, struct_table, child)
        else {
            continue;
        };
        child_offsets.remove(0);
        child_offsets = child_offsets.iter().map(|x| offsets[i] + x).collect();
        child_collection.push((i, child_offsets));
    }
    child_collection.reverse();
    for (i, child_offsets) in child_collection {
        offsets.splice(i + 1..i + 1, child_offsets);
    }
    Some(offsets)
}
