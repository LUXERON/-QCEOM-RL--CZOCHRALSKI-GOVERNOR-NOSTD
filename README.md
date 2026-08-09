# [QCEOM RL] CZOCHRALSKI-GOVERNOR — NOSTD twin

The **puller side** of the Czochralski pull governor. The map is solved
off-machine (hosted repo
[-QCEOM-RL--CZOCHRALSKI-GOVERNOR](https://github.com/LUXERON/-QCEOM-RL--CZOCHRALSKI-GOVERNOR))
once per crucible charge; the controller consumes a 1332-byte `QCCZ`
provenance image and serves `(pull-rate tier, heater tier)` commands at band
entry.

Pure `core`, zero heap, and — deliberately — **zero floating point on the
device path**. Validation is integer hashing and the map is u8 action
indices, so cross-target bit-identity is structural rather than something to
re-verify per libm.

## The accumulator crosses the boundary as an index

This is the estate's first harness whose deployed lookup is indexed by an
**integral of history**. The controller tracks accumulated slip damage and
presents it as a band index; the map answers with a command that is provably
inside the slip budget *for that history*. The device does no integration of
its own and holds no model — the physics was resolved off-machine, and the
map is the only thing that crossed.

The device-side test that matters is `accumulator_response`: it walks the map
along the damage axis and proves, on the controller, that **all 72
(length × gradient) cells change their command somewhere along that axis**,
and that **49 cells refuse outright**. A map that were flat in the damage
index would mean the third state dimension was decoration; the executor
checks rather than assumes.

## Fail-closed contract

`magic → version → CRC32 → map fingerprint → tier range → decline
monotonicity → provisioned charge hash`

- **Charge hash.** The map is bound to the crucible charge, to the
  material constants that give it meaning, **and (since 2026-08-09) to the
  codec** — the pull/heater action tiers and the state band lattice. A map
  solved for a different ingot diameter, a different melt gradient, under a
  revised material constant, or against a re-based band grid is refused
  before a single band is pulled. This refusal is load-bearing: the
  design-around posture (no online ADC loop, no in-pull adaptive melt
  model) rests on the map being re-solved per charge, so a stale map must be
  *detectable*, not merely discouraged.

  > **Image superseded 2026-08-09**: charge hash `0x3A72DADABEF89140` →
  > `0xB493141295394B69`, image CRC32 `0x550FD984` → `0xD983D89B`. The map
  > fingerprint `0x605F8ABE8A112B8F` is **unchanged** — only the provenance
  > binding moved. Host and QEMU rungs re-run on the new image; the
  > physical-silicon section below pertains to the previous one and the
  > board must be re-flashed.
- **`DECLINE` (0xFF).** Cells with no certifiable command say so explicitly.
  A pull that has banked too much damage early cannot finish the body without
  slipping; the executor stops. Terminating a body short is a recoverable
  commercial loss, a slipped 30-hour ingot is a total one.
- **Decline monotonicity.** Once the map declines at some damage band it must
  decline at every higher one — structurally guaranteed by the gate form, so
  a map that un-declines is not the map that was solved and is refused.

## Measured

| Target | Map fingerprint | Result |
|---|---|---|
| x86-64 (hosted emitter) | `0x605f8abe8a112b8f` | reference |
| QEMU mps3-an547 / Cortex-M55 | `0x605f8abe8a112b8f` | **4/4 PASS** |
| Physical STM32N657 | — | **staged, not run** (shared serialized board) |

**Three targets claimed.** See [docs/N657-RUN.md](docs/N657-RUN.md)
for the staged binary, the reset vector and the ready-to-run sequence.

## Reproduce

```bash
cargo test --release                       # 4/4 host-side

cd qemu-m55-harness
CARGO_NET_GIT_FETCH_WITH_CLI=true \
  cargo build --release --target thumbv8m.main-none-eabihf
python run_qemu_test.py \
  target/thumbv8m.main-none-eabihf/release/cz-gov-m55-harness
```

Requires `qemu-system-arm >= 8.2` (native, or inside WSL on Windows).

Regenerate the golden vector from the hosted crate whenever the model or the
map changes:

```bash
cargo run --release --bin emit_test_vector \
  > ../QCEOM-RL-CZOCHRALSKI-GOVERNOR-NOSTD/qemu-m55-harness/src/golden.rs
```


## Physical silicon — triple-target closed *(previous image)*

> Real and retained; it measured the image whose charge hash was
> `0x3A72DADABEF89140`. The map fingerprint it reports is still the
> shipped one, but the provenance header changed — re-flash and re-run to
> close the claim for the current image.

Measured on an STM32N6570-DK, 2026-08-09: **4 passed, 0 failed**
(mailbox `QCZ1`, status 2). Fingerprint `0x605f8abe8a112b8f` is
identical on x86-64, QEMU mps3-an547 and physical STM32N657.
~2.74 M cycles ~= **43 ms @ 64 MHz** including the fail-closed
refusals.

The two domain words confirm the thesis on the deployed artifact, not
just in the solver: **72** cells change pull rate in response to
*accumulated* damage, and **49** cells encode `DECLINE` (0xFF) so the
executor stops rather than serving an uncertifiable action. Full
decode: [docs/N657-RUN.md](docs/N657-RUN.md) in the NOSTD repo.
