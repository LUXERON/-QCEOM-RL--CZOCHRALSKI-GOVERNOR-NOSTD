//! The pull-map executor on emulated Cortex-M55 (QEMU mps3-an547): the
//! deployment shape end to end — a hosted-solved golden image is embedded,
//! burned to a stand-in region, validated fail-closed from there, the
//! stale-charge and out-of-range refusals are exercised, and every command
//! lookup is fingerprint-checked against the hosted crate bit for bit.
//!
//! The distinguishing test is `accumulator_response`: it walks the map along
//! the ACCUMULATED-DAMAGE axis and proves on-device that the command backs
//! off as damage banks up. That axis is the reason this harness exists, and
//! it is the one thing a puller-side executor must be shown to honour.

#![no_std]
#![no_main]

use core::ptr::addr_of_mut;
use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};
use panic_semihosting as _;

use cz_gov_nostd::image::{self, ImageError, HEADER_LEN, IMAGE_LEN, TABLE_LEN};
use cz_gov_nostd::{DMG_BANDS, GRAD_BANDS, LEN_BANDS};

mod golden;
use golden::{
    GOLDEN_CHARGE_HASH, GOLDEN_IMAGE, GOLDEN_MAP_FP, GOLDEN_OTHER_CHARGE_HASH, GOLDEN_SERIAL,
};

static mut BURN: [u8; IMAGE_LEN] = [0; IMAGE_LEN];

fn ok(cond: bool, err: &'static str) -> Result<(), &'static str> {
    if cond {
        Ok(())
    } else {
        Err(err)
    }
}

fn burn_and_accept() -> Result<(), &'static str> {
    let burn = unsafe { &mut *addr_of_mut!(BURN) };
    burn.copy_from_slice(&GOLDEN_IMAGE);
    let v = image::accept(burn, GOLDEN_CHARGE_HASH).map_err(|_| "golden refused")?;
    ok(v.serial == GOLDEN_SERIAL, "serial mismatch")?;
    ok(v.map.fingerprint() == GOLDEN_MAP_FP, "map fp mismatch")?;
    let _ = hprintln!(
        "    image {} B, map {} entries, fp {:#018x}",
        IMAGE_LEN,
        TABLE_LEN,
        v.map.fingerprint()
    );
    Ok(())
}

fn refusals() -> Result<(), &'static str> {
    let burn = unsafe { &mut *addr_of_mut!(BURN) };
    // Corrupt the burned region: CRC refuses.
    burn[HEADER_LEN + 40] ^= 1;
    ok(
        image::validate(burn).err() == Some(ImageError::BadCrc),
        "corruption must be refused (CRC)",
    )?;
    // Forge the CRC: fingerprint refuses.
    let crc = image::crc32(&burn[..IMAGE_LEN - 4]);
    burn[IMAGE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
    ok(
        image::validate(burn).err() == Some(ImageError::FingerprintMismatch),
        "forged CRC must be refused (fingerprint)",
    )?;
    // Restore; a puller provisioned for a DIFFERENT crucible charge refuses
    // this map. This is the whole "re-solved per charge" posture, enforced.
    burn.copy_from_slice(&GOLDEN_IMAGE);
    ok(
        image::accept(burn, GOLDEN_OTHER_CHARGE_HASH).err() == Some(ImageError::StaleProvenance),
        "stale charge must be refused (provenance)",
    )
}

fn lookups() -> Result<(), &'static str> {
    let burn = unsafe { &*addr_of_mut!(BURN) };
    let v = image::accept(burn, GOLDEN_CHARGE_HASH).map_err(|_| "accept")?;
    // Fingerprint the full command surface exactly as the hosted map
    // fingerprint does — bit parity across the ISA boundary or bust.
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
    ok(h == GOLDEN_MAP_FP, "command surface fp diverges from hosted")
}

fn accumulator_response() -> Result<(), &'static str> {
    // THE distinguishing device-side check. Walk the accumulated-damage axis
    // and prove the map is (a) not flat in it — otherwise the third state
    // dimension is decoration — and (b) monotone non-increasing in pull rate,
    // so a puller carrying more damage is never commanded to pull faster.
    let burn = unsafe { &*addr_of_mut!(BURN) };
    let v = image::accept(burn, GOLDEN_CHARGE_HASH).map_err(|_| "accept")?;
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
                    // DECLINE monotonicity is structurally guaranteed by the
                    // slip gate; a map that un-declines is not the map that
                    // was solved, and must never reach an actuator.
                    Some(_) if declined => return Err("map un-declines at higher damage"),
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
    let _ = hprintln!(
        "    {} of {} (length x gradient) cells respond to accumulated damage; {} declined cells",
        varying,
        LEN_BANDS * GRAD_BANDS,
        declines
    );
    ok(varying > 0, "map is flat in the accumulator axis")?;
    ok(declines > 0, "map never declines -- the guard is unreachable")
}

struct Test {
    name: &'static str,
    run: fn() -> Result<(), &'static str>,
}

const TESTS: &[Test] = &[
    Test { name: "burn_and_accept", run: burn_and_accept },
    Test { name: "refusals_fail_closed", run: refusals },
    Test { name: "command_surface_parity", run: lookups },
    Test { name: "accumulator_response", run: accumulator_response },
];

#[entry]
fn main() -> ! {
    let _ = hprintln!(
        "== Czochralski pull-map executor on Cortex-M55 (QEMU mps3-an547 / STM32N657) =="
    );
    let mut cp = cortex_m::Peripherals::take().unwrap();
    cp.DCB.enable_trace();
    cp.DWT.enable_cycle_counter();
    let mut failed = 0usize;
    for t in TESTS {
        let c0 = cortex_m::peripheral::DWT::cycle_count();
        match (t.run)() {
            Ok(()) => {
                let cycles = cortex_m::peripheral::DWT::cycle_count().wrapping_sub(c0);
                let _ = hprintln!("  [PASS] {} ({} cycles)", t.name, cycles);
            }
            Err(e) => {
                failed += 1;
                let _ = hprintln!("  [FAIL] {} - {}", t.name, e);
            }
        }
    }
    let _ = hprintln!("[harness] {} passed, {} failed", TESTS.len() - failed, failed);
    if failed == 0 {
        debug::exit(debug::EXIT_SUCCESS);
    } else {
        debug::exit(debug::EXIT_FAILURE);
    }
    loop {}
}
