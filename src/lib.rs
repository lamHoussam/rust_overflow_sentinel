use std::ffi::c_char;

use std::fmt::Debug;
use std::ptr;

use libc::IPC_CREAT;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct PtrValue {
    pub size: i32,
    pub name: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DynamicPtrTracker {
    // TODO: Need to store a hashmap of pointers to specific values (size, name, type)
    // TODO: Add mutex to value to avoid circular malloc
    ptr_values: [PtrValue; 100],
    pub ptr_count: i8,
    pub max_ptrs: i8,
    pub strcpy_bounds_violated: i8,
    pub data_leaks: i8,
    pub used_ptrs: i8,
}

impl DynamicPtrTracker {
    pub fn new() -> Self {
        DynamicPtrTracker {
            ptr_values: [PtrValue { size: 0, name: ptr::null() }; 100],
            ptr_count: 0,
            max_ptrs: 10,
            strcpy_bounds_violated: 0,
            data_leaks: 0,
            used_ptrs: 0,
        }
    }
    
    pub fn get_ptr(&self, name: *const c_char) -> PtrValue {
        for ptr_value in self.ptr_values.iter() {
            if ptr_value.name == name {
                return *ptr_value;
            }
        }
        PtrValue { size: 0, name: ptr::null() }
    }

    pub fn add_ptr(&mut self, ptr_name: *const c_char, ptr_size: i32) {
        for ptr_value in self.ptr_values.iter_mut() {
            if ptr_value.size == 0 {
                // self.ptr_values[self.ptr_count as usize] = PtrValue { size: ptr_size, name: ptr_name };
                ptr_value.size = ptr_size;
                ptr_value.name = ptr_name;
                self.ptr_count += 1;
                break;
            }
        }

        self.used_ptrs += 1;
    }

    pub fn remove_ptr(&mut self, ptr_name: *const c_char) {
        for ptr_value in self.ptr_values.iter_mut() {
            if ptr_value.name == ptr_name {
                ptr_value.size = 0;
                ptr_value.name = ptr::null();
                self.ptr_count -= 1;
            }
        }
    }

    pub fn get_number_unfreed_ptrs(&self) -> i8 {
        self.ptr_values.iter().filter(|ptr_value| ptr_value.size != 0).count() as i8
    }

    pub fn check(&mut self) {
        self.data_leaks = self.get_number_unfreed_ptrs();
    }

}

/**
 * TODO: 
 *  - >>>> Maybe Make lazystatic for CodeParser
 *  - Add appropriate values to CodeParser on malloc intercept
 *  - Intercept vulnerable functions (strcpy, strcat, ...)
 *  - Remove variable from CodeParser on free intercept
 *  - Might also check data leaks
 *  - Create Shmem on C program start
 *  - Write to Shmem on C program end
 *  - Read Shmem from Rust 
 *  - Check vulns on overriden functions (strcpy, strcat, ...)
 *  - Maybe some rust functions need to extern "C" to be used in C
 */


/*
fn get_string_from_raw_ptr(ptr: *const c_char) -> String {
     let c_str = unsafe {
        CStr::from_ptr(ptr)
    };
    let buf: &[u8] = c_str.to_bytes();
    let str_slice = std::str::from_utf8(buf).unwrap();
    str_slice.to_owned()
}
*/


// TODO: Implement write to shmem
pub fn write_to_shmem<T>(data: T, shm_key: i32) where T: Copy + Debug {
    // Write data to shared memory with id shmem_id
    // Get the size of T
    let mem_size = std::mem::size_of::<T>() as libc::size_t;
    unsafe {
        // shmflg = 0 means shm already exists
        let shmem_id = libc::shmget(shm_key, mem_size, 0o777 | IPC_CREAT);
        let ptr = libc::shmat(shmem_id, ptr::null() as *const libc::c_void, 0) as *mut T;
        // println!("Write to shmem {:?}", ptr);
        if ptr.is_null() || (ptr as isize) == -1 {
            panic!("Failed to attach to shmem on write");
        }
        ptr::write(ptr, data);
        libc::shmdt(ptr as *const libc::c_void);
    }
}

pub fn write_to_new_shmem<T>(data: T, key: i32) -> i32 where T: Copy + Debug {
    let mem_size = std::mem::size_of::<T>() as libc::size_t;
    let shm_id = unsafe { libc::shmget(key, mem_size, libc::IPC_CREAT | libc::IPC_EXCL | 0o777) };
    println!("get shmem id {}", shm_id);
    if shm_id < 0 {
        panic!("Failed to write to shmem");
    }
    let ptr = unsafe { libc::shmat(shm_id, ptr::null() as *const libc::c_void, 0) as *mut T};
    println!("attach shmem {:?}", ptr);
    if (ptr as isize) == -1{
        panic!("Failed to attach to shmem {}", std::io::Error::last_os_error());
    }

    unsafe {
        ptr::write(ptr, data);
        println!("Write to shmem {:?}", ptr);
        libc::shmdt(ptr as *const libc::c_void);
    }
    shm_id
}


// TODO: Return a value on error instead of panic
pub fn read_from_shmem<T>(shm_key: i32) -> T where T: Copy + Debug {
    let mem_size = std::mem::size_of::<T>() as libc::size_t;
    // println!("Read shmid");
    let shmem_id = unsafe {
        libc::shmget(shm_key, mem_size, 0o777 | IPC_CREAT)
    };
    // println!("Start read");
    let ptr = unsafe { libc::shmat(shmem_id,ptr::null() as *const libc::c_void, 0) } as *mut T;
    // println!("Read from shmem {:?}", ptr);
    if ptr.is_null() || (ptr as isize) == -1 {
        panic!("Failed to attach to shmem on read");
    }
    let data = unsafe { *ptr };
    // println!("Data {:?}", data);
    unsafe {
        libc::shmdt(ptr as *const libc::c_void);
        // println!("shmdt Data");
        libc::shmctl(shmem_id, libc::IPC_RMID, ptr::null_mut());
        // println!("shmctl Data");
    }
    data
}

pub fn detach_shmem(shm_key: i32) {
    let mem_size = 0;
    let shmem_id = unsafe {
        libc::shmget(shm_key, mem_size, 0o777) 
    };
    unsafe {
        libc::shmctl(shmem_id, libc::IPC_RMID, ptr::null_mut());
    }
}

/** 
 * TODO: Implement intercept for 
 *  - malloc
 *  - free
 *  - strcpy
 *  ? calloc
 *  ? realloc
 *  ? strcat
 *  ? memcpy
 *  ? memmove
 *  ? memset
 *  ? scanf
 * 
 */


#[no_mangle]
pub unsafe extern "C" fn malloc_intercept(size: i32, ptr: *mut libc::c_void) {
    let shm_key= 43;
    let mut tst_struct = read_from_shmem::<DynamicPtrTracker>(shm_key);
    // TODO: Check if shmem already open to avoid recursive malloc calls
    // Or open shmem at the beginning of C program and write at the end
    // But with this we need to add a new argument (pointer to TestStruct)
    tst_struct.add_ptr(ptr as *const c_char, size);

    write_to_shmem(tst_struct, shm_key);
}

#[no_mangle]
pub unsafe extern "C" fn free_intercept(_ptr: *mut libc::c_void) {
    let shm_key = 43;
    let mut tst_struct = read_from_shmem::<DynamicPtrTracker>(shm_key);
    // tst_struct.frees += 1;

    tst_struct.remove_ptr(_ptr as *const c_char);

    write_to_shmem(tst_struct, shm_key);
}

#[no_mangle]
pub unsafe extern "C" fn strcpy_intercept(dest: *mut libc::c_char, src: *const libc::c_char) -> i32 {
    let shm_key = 43;
    let mut tst_struct = read_from_shmem::<DynamicPtrTracker>(shm_key);

    let dest_size = libc::strlen(dest);
    let srce_size = libc::strlen(src);
    
    let mut stat = 1;
    if srce_size > dest_size {
        tst_struct.strcpy_bounds_violated += 1;
        stat = 0;
    }

    write_to_shmem(tst_struct, shm_key);
    return stat;
}
