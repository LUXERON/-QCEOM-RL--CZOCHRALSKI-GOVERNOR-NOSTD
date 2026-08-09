//! QCCZ image validation — byte-compatible with the hosted `image.rs`,
//! rebuilt on pure `core`, zero heap, zero floats.
//!
//! Layout (1332 bytes, LE): magic `"QCCZ"` u32 · format version u32 ·
//! puller serial u64 · charge-parameters hash u64 · map fingerprint u64 ·
//! 1296-byte map · CRC32 over bytes `0..1328`.

use crate::PullMap;

pub const MAGIC: u32 = 0x5A43_4351; // "QCCZ"
pub const VERSION: u32 = 1;
pub const TABLE_LEN: usize = crate::LEN_BANDS * crate::GRAD_BANDS * crate::DMG_BANDS; // 1296
pub const HEADER_LEN: usize = 32;
pub const IMAGE_LEN: usize = HEADER_LEN + TABLE_LEN + 4; // 1332

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    let mut i = 0;
    while i < data.len() {
        crc ^= data[i] as u32;
        let mut k = 0;
        while k < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            k += 1;
        }
        i += 1;
    }
    !crc
}

pub fn fingerprint(table: &[u8; TABLE_LEN]) -> u64 {
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut i = 0;
    while i < TABLE_LEN {
        h = h.rotate_left(7) ^ table[i] as u64;
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        i += 1;
    }
    h
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageError {
    BadMagic,
    BadVersion,
    BadCrc,
    FingerprintMismatch,
    /// An action index that is neither DECLINE nor a real actuator tier.
    TierOutOfRange,
    /// The map un-declines at a higher accumulated-damage band, which the
    /// slip gate makes structurally impossible — so the map is not the one
    /// that was solved.
    NonMonotoneDeclines,
    /// The map was solved for a different crucible charge, or under different
    /// material constants, than this puller was provisioned for.
    StaleProvenance,
}

#[derive(Debug)]
pub struct ValidImage {
    pub serial: u64,
    pub charge_hash: u64,
    pub map: PullMap,
}

fn u32_at(img: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([img[i], img[i + 1], img[i + 2], img[i + 3]])
}

fn u64_at(img: &[u8], i: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&img[i..i + 8]);
    u64::from_le_bytes(b)
}

/// Structural validation: magic → version → CRC → fingerprint → tier range.
pub fn validate(img: &[u8]) -> Result<ValidImage, ImageError> {
    if img.len() < IMAGE_LEN || u32_at(img, 0) != MAGIC {
        return Err(ImageError::BadMagic);
    }
    if u32_at(img, 4) != VERSION {
        return Err(ImageError::BadVersion);
    }
    if crc32(&img[..IMAGE_LEN - 4]) != u32_at(img, IMAGE_LEN - 4) {
        return Err(ImageError::BadCrc);
    }
    let mut actions = [0u8; TABLE_LEN];
    actions.copy_from_slice(&img[HEADER_LEN..HEADER_LEN + TABLE_LEN]);
    if fingerprint(&actions) != u64_at(img, 24) {
        return Err(ImageError::FingerprintMismatch);
    }
    let map = PullMap { actions };
    if !map.tiers_in_range() {
        return Err(ImageError::TierOutOfRange);
    }
    if !map.declines_are_monotone() {
        return Err(ImageError::NonMonotoneDeclines);
    }
    Ok(ValidImage { serial: u64_at(img, 8), charge_hash: u64_at(img, 16), map })
}

/// Full puller-side acceptance: structural validation PLUS the provisioned
/// charge expectation. This is the call a puller makes before the first band.
pub fn accept(img: &[u8], expected_charge_hash: u64) -> Result<ValidImage, ImageError> {
    let v = validate(img)?;
    if v.charge_hash != expected_charge_hash {
        return Err(ImageError::StaleProvenance);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Golden vector emitted by the hosted crate (worst-case 200 mm charge).
    include!("../qemu-m55-harness/src/golden.rs");

    #[test]
    fn crc32_known_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn golden_image_parity_with_hosted() {
        let v = accept(&GOLDEN_IMAGE, GOLDEN_CHARGE_HASH).expect("golden accepts");
        assert_eq!(v.serial, GOLDEN_SERIAL);
        assert_eq!(v.map.fingerprint(), GOLDEN_MAP_FP);
        // Every entry decodes to a real actuator tier.
        assert!(v.map.tiers_in_range());
        for lb in 0..crate::LEN_BANDS {
            for gb in 0..crate::GRAD_BANDS {
                for db in 0..crate::DMG_BANDS {
                    if let Some(c) = v.map.command(lb, gb, db) {
                        assert!((c.pull_tier as usize) < crate::PULL_TIERS);
                        assert!((c.heater_tier as usize) < crate::HEATER_TIERS);
                    }
                }
            }
        }
        assert!(v.map.declines_are_monotone());
    }

    #[test]
    fn the_accumulator_is_visible_in_the_deployed_map() {
        // The whole reason this harness exists: the command depends on the
        // damage ALREADY accumulated, not just on where the ingot is. If the
        // map were flat in the damage index, the third state dimension would
        // be decoration and this crate would be shipping a lie.
        let v = accept(&GOLDEN_IMAGE, GOLDEN_CHARGE_HASH).expect("golden accepts");
        let mut varies = 0usize;
        let mut declines = 0usize;
        for lb in 0..crate::LEN_BANDS {
            for gb in 0..crate::GRAD_BANDS {
                let first = v.map.command(lb, gb, 0);
                if (1..crate::DMG_BANDS).any(|db| v.map.command(lb, gb, db) != first) {
                    varies += 1;
                }
                declines += (0..crate::DMG_BANDS)
                    .filter(|&db| v.map.command(lb, gb, db).is_none())
                    .count();
            }
        }
        assert!(
            varies > 0,
            "the deployed map must respond to accumulated damage"
        );
        // A pull that has banked enough damage must be refused outright --
        // otherwise the guard is unreachable and the accumulator is not
        // actually binding anywhere in the deployed artifact.
        assert!(declines > 0, "the map must decline somewhere on the damage axis");
    }

    #[test]
    fn corruption_and_staleness_are_refused() {
        let mut bad = GOLDEN_IMAGE;
        bad[HEADER_LEN + 40] ^= 1;
        assert_eq!(validate(&bad).unwrap_err(), ImageError::BadCrc);
        let crc = crc32(&bad[..IMAGE_LEN - 4]);
        bad[IMAGE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(validate(&bad).unwrap_err(), ImageError::FingerprintMismatch);
        // A map for the NOMINAL charge must be REFUSED by a puller
        // provisioned for the worst-case charge, and vice versa.
        assert_eq!(
            accept(&GOLDEN_IMAGE, GOLDEN_OTHER_CHARGE_HASH).unwrap_err(),
            ImageError::StaleProvenance
        );
        // An out-of-range action index that is otherwise self-consistent is
        // still refused: CRC and fingerprint prove integrity, not meaning.
        let mut oor = GOLDEN_IMAGE;
        oor[HEADER_LEN] = 200;
        let mut acts = [0u8; TABLE_LEN];
        acts.copy_from_slice(&oor[HEADER_LEN..HEADER_LEN + TABLE_LEN]);
        oor[24..32].copy_from_slice(&fingerprint(&acts).to_le_bytes());
        let crc = crc32(&oor[..IMAGE_LEN - 4]);
        oor[IMAGE_LEN - 4..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(validate(&oor).unwrap_err(), ImageError::TierOutOfRange);
    }
}
