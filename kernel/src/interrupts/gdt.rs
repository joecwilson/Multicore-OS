//! Global Descriptor Table (GDT) setup and initialization.
//!
//! This module handles:
//! - GDT creation with kernel and user segments
//! - Task State Segment (TSS) setup for each CPU core
//! - Interrupt Stack Table (IST) configuration
//! - Segment register initialization

// Will remove after getting context switching
// Right now user code/data is not used
#![allow(dead_code)]

use lazy_static::lazy_static;
use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::PrivilegeLevel;
use x86_64::VirtAddr;

use crate::constants::gdt::{DOUBLE_FAULT_IST_INDEX, IST_STACK_SIZE};
use crate::constants::MAX_CORES;

/// Number of base GDT entries: null descriptor + kernel code/data + user code/data
const BASE_ENTRIES: usize = 5;

/// Number of GDT entries needed per TSS (each TSS requires 16 bytes in long mode)
const TSS_ENTRIES_PER_CORE: usize = 2;

/// Total number of GDT entries needed
const GDT_ENTRIES: usize = BASE_ENTRIES + TSS_ENTRIES_PER_CORE * MAX_CORES;

lazy_static! {
    /// Task State Segments (TSS) for each CPU core.
    /// Each TSS contains:
    /// - Interrupt Stack Table (IST) for handling exceptions
    /// - Kernel stack pointer (RSP0) for privilege level changes
    static ref TSSS: [TaskStateSegment; MAX_CORES] = {
        static mut STACKS: [[u8; IST_STACK_SIZE]; MAX_CORES] = [[0; IST_STACK_SIZE]; MAX_CORES];
        let mut tsss = [TaskStateSegment::new(); MAX_CORES];

        for (i, tss) in tsss.iter_mut().enumerate() {
            unsafe {
                let stack_start = VirtAddr::from_ptr(&STACKS[i]);
                let stack_end = stack_start + IST_STACK_SIZE as u64;

                tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stack_end;
            }
        }
        tsss
    };

    /// Global Descriptor Table and segment selectors.
    /// Contains:
    /// - Kernel code and data segments
    /// - User code and data segments (currently unused)
    /// - TSS descriptors for each CPU core
    static ref GDT: (GlobalDescriptorTable<GDT_ENTRIES>, Selectors) = {
        let mut gdt = GlobalDescriptorTable::<GDT_ENTRIES>::empty();

        // Add segments
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        let user_data_selector = gdt.append(Descriptor::user_data_segment());

        let mut tss_selectors = [SegmentSelector::new(0, PrivilegeLevel::Ring0); MAX_CORES];

        for i in 0..MAX_CORES {
            tss_selectors[i] = gdt.append(Descriptor::tss_segment(&TSSS[i]));
        }

        (gdt, Selectors {
            code_selector,
            data_selector,
            user_code_selector,
            user_data_selector,
            tss_selectors,
        })
    };
}

/// Collection of segment selectors for kernel and user segments, plus TSS selectors.
#[derive(Debug)]
struct Selectors {
    code_selector: SegmentSelector,
    data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    tss_selectors: [SegmentSelector; MAX_CORES],
}

/// Initialize GDT and segment registers for the specified CPU core.
///
/// # Arguments
/// * `cpu_id` - ID of the CPU being initialized
///
/// # Panics
/// Panics if cpu_id >= MAX_CORES
pub fn init(cpu_id: u32) {
    assert!(cpu_id < MAX_CORES as u32, "CPU ID exceeds MAX_CORES");

    GDT.0.load();

    unsafe {
        // Set up segment registers with appropriate selectors
        CS::set_reg(GDT.1.code_selector);

        ES::set_reg(GDT.1.data_selector);
        DS::set_reg(GDT.1.data_selector);
        SS::set_reg(GDT.1.data_selector);
        FS::set_reg(GDT.1.data_selector);
        GS::set_reg(GDT.1.data_selector);

        load_tss(GDT.1.tss_selectors[cpu_id as usize]);
    }
}
