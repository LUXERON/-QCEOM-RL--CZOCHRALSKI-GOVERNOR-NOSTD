//! [QCEOM RL] Czochralski pull governor — NOSTD twin.
//!
//! The puller side of the pipeline. The pull map is solved OFF-machine
//! (hosted repo `LUXERON/-QCEOM-RL--CZOCHRALSKI-GOVERNOR`) once per crucible
//! charge; the machine consumes a 1332-byte QCCZ provenance image and serves
//! `(pull-rate tier, heater tier)` commands at band entry. This crate is
//! therefore pure `core`, zero heap, and — deliberately — **zero floating
//! point on the device path**: validation is integer hashing and the map is
//! u8 action indices, so cross-target bit-identity is structural rather than
//! something to re-verify per libm.
//!
//! # The accumulator crosses the boundary as an INDEX
//!
//! This is the estate's first harness whose deployed lookup is indexed by an
//! *integral of history*. The machine tracks accumulated slip damage and
//! presents it as a band index; the map answers with the command that is
//! provably inside the slip budget **for that history**. The device does no
//! integration of its own and holds no model — the physics was resolved
//! off-machine and the map is the only thing that crossed.
//!
//! Fail-closed contract (byte-compatible with the hosted `image.rs`):
//! magic → version → CRC32 → map fingerprint, then the provisioned
//! charge-hash comparison. A map solved for a different crucible charge, a
//! different ingot diameter, or under revised material constants is refused
//! before a single band is pulled. That refusal is load-bearing: the
//! design-around posture (no online ADC loop, no in-pull adaptive melt model)
//! rests on the map being re-solved per charge, so a stale map must be
//! *detectable*, not merely discouraged.

#![cfg_attr(not(test), no_std)]

pub mod image;

/// Length bands crossed to grow the ingot body.
pub const LEN_BANDS: usize = 12;
/// Interface-gradient bands (aligned 1:1 with the pull-rate tiers).
pub const GRAD_BANDS: usize = 6;
/// Accumulated-damage bands over the declared planning guard.
pub const DMG_BANDS: usize = 18;
/// Pull-rate tiers the action index decodes into.
pub const PULL_TIERS: usize = 6;
/// Heater tiers the action index decodes into.
pub const HEATER_TIERS: usize = 3;

/// The executable pull map: one action index per
/// (length band × gradient band × damage band).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullMap {
    pub actions: [u8; image::TABLE_LEN],
}

/// A decoded puller command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// Pull-rate tier index, 0..PULL_TIERS.
    pub pull_tier: u8,
    /// Heater-power tier index, 0..HEATER_TIERS.
    pub heater_tier: u8,
}

/// Reserved map entry meaning **no certifiable command exists — decline**.
///
/// Some (length, gradient, damage) cells have no gate-clean action: a pull
/// that has banked too much damage this early cannot finish the body without
/// slipping. The solved map records that explicitly so the puller STOPS
/// rather than improvising. Terminating the body short is a recoverable
/// commercial loss; a slipped 30-hour ingot is a total one.
pub const DECLINE: u8 = 0xFF;

impl PullMap {
    /// The runtime of the whole product: one array lookup at band entry,
    /// indexed by ingot length, interface gradient, and — the point of this
    /// harness — the damage the pull has ALREADY accumulated.
    ///
    /// `None` means the map declines the state: end the body here.
    #[inline]
    pub fn command(
        &self,
        length_band: usize,
        grad_band: usize,
        damage_band: usize,
    ) -> Option<Command> {
        let a = self.actions[(length_band * GRAD_BANDS + grad_band) * DMG_BANDS + damage_band];
        if a == DECLINE {
            return None;
        }
        Some(Command { pull_tier: a / HEATER_TIERS as u8, heater_tier: a % HEATER_TIERS as u8 })
    }

    /// Every entry must be either DECLINE or decode to a real tier pair. A map
    /// that indexes off the end of the actuator ladder is a corrupt map that
    /// passed CRC, so this is checked before the map is trusted.
    pub fn tiers_in_range(&self) -> bool {
        let mut i = 0;
        while i < image::TABLE_LEN {
            let a = self.actions[i];
            if a != DECLINE && a as usize >= PULL_TIERS * HEATER_TIERS {
                return false;
            }
            i += 1;
        }
        true
    }

    /// DECLINE monotonicity: once the map declines at some accumulated-damage
    /// band it must decline at every higher one. This is structurally
    /// guaranteed by the gate (`top(db) + Δ > guard` with `top` increasing),
    /// so a map that un-declines is a corrupt or mis-packed map and must not
    /// be trusted on a puller.
    pub fn declines_are_monotone(&self) -> bool {
        let mut lb = 0;
        while lb < LEN_BANDS {
            let mut gb = 0;
            while gb < GRAD_BANDS {
                let mut declined = false;
                let mut db = 0;
                while db < DMG_BANDS {
                    let a = self.actions[(lb * GRAD_BANDS + gb) * DMG_BANDS + db];
                    if a == DECLINE {
                        declined = true;
                    } else if declined {
                        return false;
                    }
                    db += 1;
                }
                gb += 1;
            }
            lb += 1;
        }
        true
    }

    pub fn fingerprint(&self) -> u64 {
        image::fingerprint(&self.actions)
    }
}
