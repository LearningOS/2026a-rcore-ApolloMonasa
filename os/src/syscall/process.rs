//! Process management syscalls

use crate::config::{PAGE_SIZE, TRAP_CONTEXT_BASE};
use crate::mm::{translated_byte_buffer_checked, MapPermission, VirtAddr};
use crate::task::{change_program_brk, current_user_token, exit_current_and_run_next, get_syscall_times, handle_page_fault_current, mmap_current, munmap_current, suspend_current_and_run_next};
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let tv = TimeVal { sec: us / 1_000_000, usec: us % 1_000_000 };
    if let Some(buffers) = translated_byte_buffer_checked(
        current_user_token(), _ts as *const u8, core::mem::size_of::<TimeVal>(), true,
    ) {
        let src = unsafe {
            core::slice::from_raw_parts(
                &tv as *const TimeVal as *const u8,
                core::mem::size_of::<TimeVal>(),
            )
        };
        let mut pos = 0;
        for buffer in buffers {
            for bytes in buffer.iter_mut() {
                *bytes = src[pos];
                pos += 1;
            }
        }
        0
    } else {
        -1
    }
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// trace: read one byte / write one byte at user virtual address `id`
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    match _trace_request {
        0 => {
            // Read: Return the bytes itself or -1 when failing
            // 惰性 mmap 区首次访问前无映射：先按用户态语义补帧，再重试
            let mut buffers = translated_byte_buffer_checked(token, _id as *const u8, 1, false);
            if buffers.is_none()
                && handle_page_fault_current(VirtAddr::from(_id), MapPermission::R)
            {
                buffers = translated_byte_buffer_checked(token, _id as *const u8, 1, false);
            }
            match buffers {
                Some(buffers) => buffers[0][0] as isize,
                None => -1,
            }
        }
        1 => {
            // Write: Return  0 when writing the LSB(8) of data, or -1 when failing
            let mut buffers = translated_byte_buffer_checked(token, _id as *const u8, 1, true);
            if buffers.is_none()
                && handle_page_fault_current(VirtAddr::from(_id), MapPermission::W)
            {
                buffers = translated_byte_buffer_checked(token, _id as *const u8, 1, true);
            }
            match buffers {
                Some(mut buffers) => {
                    buffers[0][0] = _data as u8;
                    0
                }
                None => -1,
            }
        }
        2 => get_syscall_times(_id) as isize,
        _ => -1isize,
    }
}

// YOUR JOB: Implement mmap.
/// 在 Linux 中主要用于在内存中映射文件， 本次实验简化它的功能，仅用于申请内存。
/// syscall ID：222
/// 申请长度为 len 字节的物理内存（不要求实际物理内存位置，可以随便找一块），将其映射到 start 开始的虚存，内存页属性为 prot
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    // trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    trace!("kernel: sys_mmap");
    // 校验：start 页对齐；len 非零；port 必须是 R(1)|W(2) 的非空组合
    if _start % PAGE_SIZE != 0 || _len == 0 || _port == 0 || _port & !0x3 != 0 {
        return -1;
    }
    // 区间不得越过 TRAP_CONTEXT_BASE：那里往上是 TrapContext 页和跳板页，
    // 撞上跳板会让 PageTable::map 的 assert 直接打崩内核
    if _start >= TRAP_CONTEXT_BASE || TRAP_CONTEXT_BASE - _start < _len {
        return -1;
    }
    // 用户态 port 位定义（bit0=R, bit1=W）与内核 MapPermission（R=1<<1, W=1<<2）不同，
    // 必须逐位翻译，另加 U 位表示用户态可访问
    let mut perm = MapPermission::U;
    if _port & 0x1 != 0 {
        perm |= MapPermission::R;
    }
    if _port & 0x2 != 0 {
        perm |= MapPermission::W;
    }
    mmap_current(_start, _len, perm)
}

// YOUR JOB: Implement munmap.
/// syscall ID：215
/// 取消到 [start, start + len) 虚存的映射
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    // trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    trace!("kernel: sys_munmap");
    // munmap 要求 start 与 len 都页对齐
    if _start % PAGE_SIZE != 0 || _len == 0 || _len % PAGE_SIZE != 0 {
        return -1;
    }
    munmap_current(_start, _len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
