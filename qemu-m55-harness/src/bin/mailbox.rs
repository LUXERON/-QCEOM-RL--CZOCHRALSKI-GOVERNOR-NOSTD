//! N657 mailbox variant: no semihosting — results + DWT cycles to AXISRAM
//! @ 0x3417_8000, loaded as raw .bin via CubeProgrammer, started with
//! -halt → -coreReg → -run (the proven recipe). The golden pull-map image is
//! burned to 0x3417_9000 and validated FROM there; a valid image is left for
//! post-run inspection.
//!
//! Mailbox: [0] magic "QCZ1" 0x315A4351 · [1] status 1/2/3 · [2] passed ·
//! [3] failed · [4,5] map fingerprint · [6] image CRC32 · [7..11] cycles per
//! test · [11] progress · [12] fine-grained marker inside test 1 · [13] cells
//! responding to the accumulator · [14] declined cells.
//!
//! The marker word is kept from the fast-charge/hearing bring-ups: one
//! first-load run wedged mid-accept and did not reproduce after a fresh load
//! (the recipe's known first-load flakiness). Markers stay as cheap diagnosis.

#![no_std]
#![no_main]

use core::ptr::write_volatile;
use cortex_m_rt::entry;
use panic_semihosting as _;

use cz_gov_nostd::image::{self, ImageError, HEADER_LEN, IMAGE_LEN};
use cz_gov_nostd::{DMG_BANDS, GRAD_BANDS, LEN_BANDS};

#[path = "../golden.rs"]
mod golden;
use golden::{
    GOLDEN_CHARGE_HASH, GOLDEN_IMAGE, GOLDEN_MAP_FP, GOLDEN_OTHER_CHARGE_HASH, GOLDEN_SERIAL,
};

const MAILBOX: *mut u32 = 0x3417_8000 as *mut u32;
const BURN_REGION: *mut u8 = 0x3417_9000 as *mut u8;

fn mb(idx: usize, val: u32) {
    unsafe { write_volatile(MAILBOX.add(idx), val) }
}

fn burned() -> &'static mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(BURN_REGION, IMAGE_LEN) }
}

fn burn_and_accept() -> bool {
    let b = burned();
    for (dst, &src) in b.iter_mut().zip(GOLDEN_IMAGE.iter()) {
        unsafe { write_volatile(dst as *mut u8, src) }
    }
    mb(12, 1); // burn done
    mb(6, image::crc32(&b[..IMAGE_LEN - 4]));
    mb(12, 2); // crc done
    let r = image::validate(b);
    mb(12, 3); // validate returned
    let v = match r {
        Ok(v) => v,
        Err(_) => return false,
    };
    mb(12, 4);
    if v.charge_hash != GOLDEN_CHARGE_HASH {
        return false;
    }
    mb(12, 5);
    let fp = v.map.fingerprint();
    mb(4, fp as u32);
    mb(5, (fp >> 32) as u32);
    mb(12, 6);
    v.serial == GOLDEN_SERIAL && fp == GOLDEN_MAP_FP
}

fn refusals() -> bool {
    let b = burned();
    b[HEADER_LEN + 40] ^= 1;
    let crc_reject = image::validate(b).err() == Some(ImageError::BadCrc);
    let crc = image::crc32(&b[..IMAGE_LEN - 4]);
    b[IMAGE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    let fp_reject = image::validate(b).err() == Some(ImageError::FingerprintMismatch);
    b.copy_from_slice(&GOLDEN_IMAGE);
    let stale_reject =
        image::accept(b, GOLDEN_OTHER_CHARGE_HASH).err() == Some(ImageError::StaleProvenance);
    crc_reject && fp_reject && stale_reject
}

fn lookups() -> bool {
    let b = burned();
    let v = match image::accept(b, GOLDEN_CHARGE_HASH) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
    for lb in 0..LEN_BANDS {
        for gb in 0..GRAD_BANDS {
            for db in 0..DMG_BANDS {
                let a = match v.map.command(lb, gb, db) {
                    Some(c) => c.pull_tier * 3 + c.heater_tier,
                    None => cz_gov_nostd::DECLINE,
                };
                h = h.rotate_left(7) ^ a as u64;
                h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
            }
        }
    }
    h == GOLDEN_MAP_FP
}

fn accumulator_response() -> bool {
    // The distinguishing device-side check: the deployed map must respond to
    // ACCUMULATED damage, and must never un-decline as damage grows.
    let b = burned();
    let v = match image::accept(b, GOLDEN_CHARGE_HASH) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut varying = 0u32;
    let mut declines = 0u32;
    for lb in 0..LEN_BANDS {
        for gb in 0..GRAD_BANDS {
            let base = v.map.command(lb, gb, 0);
            let mut declined = false;
            let mut differs = false;
            for db in 0..DMG_BANDS {
                let c = v.map.command(lb, gb, db);
                match c {
                    None => {
                        declined = true;
                        declines += 1;
                    }
                    Some(_) if declined => return false,
                    Some(_) => {}
                }
                if c != base {
                    differs = true;
                }
            }
            if differs {
                varying += 1;
            }
        }
    }
    mb(13, varying);
    mb(14, declines);
    varying > 0 && declines > 0
}

#[entry]
fn main() -> ! {
    mb(0, 0x315A_4351); // "QCZ1"
    mb(1, 1);
    for i in 2..15 {
        mb(i, 0);
    }
    let mut cp = cortex_m::Peripherals::take().unwrap();
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();

    let mut passed = 0u32;
    let mut failed = 0u32;
    let tests: [(fn() -> bool, usize); 4] = [
        (burn_and_accept, 7),
        (refusals, 8),
        (lookups, 9),
        (accumulator_response, 10),
    ];
    for (i, (test, cyc_idx)) in tests.iter().enumerate() {
        mb(11, i as u32 + 1);
        let c0 = cortex_m::peripheral::DWT::cycle_count();
        let okk = test();
        let cycles = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(c0);
        mb(*cyc_idx, cycles);
        if okk {
            passed += 1;
        } else {
            failed += 1;
        }
        mb(2, passed);
        mb(3, failed);
    }
    mb(1, if failed == 0 { 2 } else { 3 });
    loop {
        cortex_m::asm::nop();
    }
}
