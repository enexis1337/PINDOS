extern crate alloc;

use alloc::boxed::Box;
use core::marker::PhantomData;
use core::ptr;

use crate::acpi::madt::Madt;
use crate::arch::x86_64::apic;
use crate::kprintln;
use crate::sched::Scheduler;
use crate::sched::task::STACK_SIZE;
use crate::mm::{map_page, translate, PhysFrame, PageFlags, PHYSICAL_ALLOCATOR};

const MAX_CPUS: usize = 16;
const AP_TRAMPOLINE_PHYS: u64 = 0x8000;
const PER_CPU_GS_BASE_MSR: u32 = 0xC0000101;

const ICR_DELIVERY_MODE_INIT: u32 = 0x500;
const ICR_DELIVERY_MODE_STARTUP: u32 = 0x600;
const ICR_LEVEL_ASSERT: u32 = 1 << 14;
const ICR_TRIGGER_MODE_LEVEL: u32 = 1 << 15;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PerCpuData {
    pub cpu_id: u32,
    pub _reserved: u32,
    pub scheduler: *mut Scheduler,
    pub stack_top: u64,
}

static mut PER_CPU_DATA: [PerCpuData; MAX_CPUS] = [PerCpuData {
    cpu_id: 0,
    _reserved: 0,
    scheduler: ptr::null_mut(),
    stack_top: 0,
}; MAX_CPUS];

static mut AP_STACKS: [*mut [u8; STACK_SIZE]; MAX_CPUS] = [ptr::null_mut(); MAX_CPUS];

pub struct PerCpu<T> {
    offset: usize,
    _marker: PhantomData<fn() -> T>,
}

unsafe impl<T> Send for PerCpu<T> {}
unsafe impl<T> Sync for PerCpu<T> {}

impl<T> PerCpu<T> {
    pub const fn new(offset: usize) -> Self {
        Self {
            offset,
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> &mut T {
        let addr: usize;
        unsafe {
            core::arch::asm!(
                "lea {addr}, gs:[{offset}]",
                addr = out(reg) addr,
                offset = in(reg) self.offset,
                options(nomem, nostack, preserves_flags)
            );
            &mut *(addr as *mut T)
        }
    }
}

macro_rules! thread_local {
    (static $name:ident: $ty:ty = $offset:expr;) => {
        pub static $name: PerCpu<$ty> = PerCpu::new($offset);
    };
}

const CPU_ID_OFFSET: usize = 0;
const CPU_SCHEDULER_OFFSET: usize = 8;
const CPU_STACK_TOP_OFFSET: usize = 16;

thread_local! {
    static CPU_ID: u32 = CPU_ID_OFFSET;
}
thread_local! {
    static CPU_SCHEDULER: *mut Scheduler = CPU_SCHEDULER_OFFSET;
}
thread_local! {
    static CPU_STACK_TOP: u64 = CPU_STACK_TOP_OFFSET;
}

pub fn cpu_id() -> u32 {
    *CPU_ID.get()
}

pub fn current_scheduler() -> *mut Scheduler {
    *CPU_SCHEDULER.get()
}

pub fn per_cpu_stack_top() -> u64 {
    *CPU_STACK_TOP.get()
}

pub fn init_aps(rsdp_addr: u64) {
    apic::init_local_apic();
    prepare_trampoline();

    if let Some(madt) = Madt::from_rsdp(rsdp_addr) {
        let bsp_id = apic::local_apic_id();
        for local in madt.local_apics() {
            if local.flags & 1 == 0 {
                continue;
            }
            if local.apic_id == bsp_id {
                continue;
            }
            if let Err(_) = start_ap(local.apic_id) {
                kprintln!("[smp] failed to start AP {}", local.apic_id);
            }
        }
    }
}

fn prepare_trampoline() {
    if translate(AP_TRAMPOLINE_PHYS).is_none() {
        let mut allocator = PHYSICAL_ALLOCATOR.lock();
        let _ = unsafe {
            map_page(
                AP_TRAMPOLINE_PHYS,
                PhysFrame::new(AP_TRAMPOLINE_PHYS),
                PageFlags::PRESENT | PageFlags::WRITABLE,
                &mut *allocator,
            )
        };
    }

    unsafe {
        let dest = AP_TRAMPOLINE_PHYS as *mut u8;
        let trampoline: [u8; 19] = [
            0xFA, // cli
            0xEB, 0xFE, // jmp $-2
            0x90, 0x90, 0x90, 0x90, 0x90,
            0x90, 0x90, 0x90, 0x90, 0x90,
            0x90, 0x90, 0x90, 0x90, 0x90,
            0x90,
        ];
        ptr::copy_nonoverlapping(trampoline.as_ptr(), dest, trampoline.len());
    }
}

fn start_ap(apic_id: u8) -> Result<(), ()> {
    if apic_id as usize >= MAX_CPUS {
        return Err(());
    }

    let stack = Box::new([0u8; STACK_SIZE]);
    let raw_stack = Box::into_raw(stack);
    let stack_top = unsafe { (*raw_stack).as_mut_ptr().add(STACK_SIZE) } as u64;
    let stack_top = stack_top & !0x0F;

    unsafe {
        AP_STACKS[apic_id as usize] = raw_stack;
        PER_CPU_DATA[apic_id as usize] = PerCpuData {
            cpu_id: apic_id as u32,
            _reserved: 0,
            scheduler: ptr::null_mut(),
            stack_top,
        };
    }

    send_init_ipi(apic_id);
    io_wait();
    send_startup_ipi(apic_id);
    io_wait();
    send_startup_ipi(apic_id);
    Ok(())
}

fn send_init_ipi(apic_id: u8) {
    let low = ICR_DELIVERY_MODE_INIT | ICR_LEVEL_ASSERT | ICR_TRIGGER_MODE_LEVEL;
    apic::send_icr(apic_id, low);
}

fn send_startup_ipi(apic_id: u8) {
    let vector = (AP_TRAMPOLINE_PHYS >> 12) as u32 & 0xFF;
    let low = vector | ICR_DELIVERY_MODE_STARTUP;
    apic::send_icr(apic_id, low);
}

fn io_wait() {
    unsafe {
        core::arch::asm!(
            "out 0x80, al",
            in("al") 0u8,
            options(nomem, nostack, preserves_flags),
        );
    }
}

pub unsafe fn set_gs_base(base: u64) {
    unsafe { write_msr(PER_CPU_GS_BASE_MSR, base) };
}

unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") low,
            in("edx") high,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[no_mangle]
pub extern "C" fn ap_entry() -> ! {
    let apic_id = apic::local_apic_id();
    let base = unsafe { &PER_CPU_DATA[apic_id as usize] as *const PerCpuData as u64 };
    unsafe { set_gs_base(base) };

    apic::init_local_apic();

    let scheduler = Box::leak(Box::new(Scheduler::new()));
    unsafe {
        PER_CPU_DATA[apic_id as usize].scheduler = scheduler as *mut Scheduler;
    }

    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}
