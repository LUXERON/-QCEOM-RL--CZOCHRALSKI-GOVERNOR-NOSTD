# STM32N657 Physical Run — Czochralski Pull-Map Executor

**Status: REBUILT AND STAGED (second image), NOT YET EXECUTED ON
SILICON.** The board is a shared serialized resource under lead-dev
control, so this harness stops at the built artifact. Everything below is
ready to run verbatim.

> **The measured-result section at the bottom of this file is
> SUPERSEDED.** A 2026-08-09 estate-wide audit found the params hash
> guarded the physics but not the **codec**: the image is a bare
> action-index byte per state, so re-basing a band grid at constant band
> count, or re-declaring `PULL_TIERS` / `HEATER_T_AMB`, would produce a
> same-length image with an unchanged hash that misindexes every lookup.
> `PULL_TIERS`, `HEATER_T_AMB`, `HEATER_PSI`, `L_TOTAL`, `THERMO`,
> `RAMP_BANDS`, the band widths and the band counts are now hashed.
>
> | Field | Previous image | Current image |
> |---|---|---|
> | Charge hash (header offset 16) | `0x3A72DADABEF89140` | **`0xB493141295394B69`** |
> | Stale-charge demo constant | `0xE260DB72161AB5BC` | **`0xAD27333AF276C91E`** |
> | Image CRC32 (mailbox word [6]) | `0x550FD984` | **`0xD983D89B`** |
> | Map fingerprint (words [4,5]) | `0x605F8ABE8A112B8F` | **unchanged** |
>
> The solved map did not change — only its provenance binding, and
> therefore the image header and CRC. Host parity 4/4 and QEMU
> mps3-an547 4/4 re-run on the new image. **The lead developer must
> re-flash `mailbox_burn.bin` and re-run.**

## What is staged

| Item | Value |
|---|---|
| Binary | `qemu-m55-harness/mailbox_burn.bin` |
| Size | **13,732 bytes** (includes the 1,332-byte golden `QCCZ` image) |
| Built with | `THERMAL_N657=1` → `memory-n657.x` (RAM-only board map) |
| Load address | `0x3410_0000` (second AXISRAM megabyte — the first does not accept bulk SWD writes) |
| Initial MSP | `0x3420_0000` |
| **Reset vector** | **`0x3410_07C1`** (thumb bit set; little-endian u32 at file offset 4) |
| Mailbox | `0x3417_8000`, magic `QCZ1` = `0x315A_4351` |
| Burn region | `0x3417_9000` (the golden image is written there and validated FROM there) |

## Reproduce the build

```bash
cd qemu-m55-harness
THERMAL_N657=1 CARGO_NET_GIT_FETCH_WITH_CLI=true \
  cargo build --release --target thumbv8m.main-none-eabihf --bin mailbox
"$HOME/.rustup/toolchains/nightly-x86_64-pc-windows-msvc/lib/rustlib/x86_64-pc-windows-msvc/bin/llvm-objcopy.exe" \
  -O binary target/thumbv8m.main-none-eabihf/release/mailbox mailbox_burn.bin
```

## The run sequence (ready to execute — proven recipe, do not re-derive)

Raw `.bin`, **not** ELF. Start via `-halt` → `-coreReg` → `-run`.

```bash
STM32_Programmer_CLI -c port=SWD mode=HOTPLUG -q \
  -w mailbox_burn.bin 0x34100000 \
  -halt -coreReg xPSR=0x01000000 MSP=0x34200000 PC=0x341007c1 -run

# then read the mailbox — CHECK THE MAGIC FIRST, old data persists across loads
STM32_Programmer_CLI -c port=SWD mode=HOTPLUG -q -r32 0x34178000 60
```

## Mailbox decoding

| Word | Offset | Meaning | Expected |
|---|---|---|---|
| [0] | `0x34178000` | magic `QCZ1` | `0x315A4351` |
| [1] | `+0x04` | status | **2** = all passed (1 = running, 3 = failures) |
| [2] | `+0x08` | tests passed | **4** |
| [3] | `+0x0C` | tests failed | **0** |
| [4] | `+0x10` | map fingerprint, low word | `0x8A112B8F` |
| [5] | `+0x14` | map fingerprint, high word | `0x605F8ABE` |
| [6] | `+0x18` | image CRC32 recomputed on silicon | `0xD983D89B` |
| [7] | `+0x1C` | `burn_and_accept` DWT cycles | — |
| [8] | `+0x20` | `refusals` DWT cycles | — |
| [9] | `+0x24` | `lookups` DWT cycles | — |
| [10] | `+0x28` | `accumulator_response` DWT cycles | — |
| [11] | `+0x2C` | progress (index of the running test) | 4 when complete |
| [12] | `+0x30` | fine-grained marker inside test 1 | 6 when complete |
| [13] | `+0x34` | (length × gradient) cells that respond to accumulated damage | **72** |
| [14] | `+0x38` | declined cells on the damage axis | **49** |

Words [13] and [14] are the ones worth reading twice. They are the
device-side evidence that the deployed artifact is genuinely a function of
**accumulated stress history** and not merely of position along the ingot:
all 72 (length × gradient) cells change their command somewhere along the
damage axis, and 49 cells refuse outright — a pull that has banked too much
damage is told to stop, on the controller, without any model on board.

## The acceptance claim this run would establish

The map fingerprint `0x605f8abe8a112b8f` is already identical on **x86-64**
(the hosted emitter, `cargo run --bin emit_test_vector`) and on **QEMU
mps3-an547 / Cortex-M55** (4/4 tests). A physical N657 run reporting the
same fingerprint at words [4,5] would close the triple-target
bit-determinism claim for this harness. Until then the README claims **two
targets, not three**.

## Known bring-up notes (inherited, not re-derived)

- The first AXISRAM megabyte (`0x3400_0000`) rejects bulk SWD downloads under
  CubeProgrammer; everything therefore lives at `0x3410_0000+`.
- One first-load run in an earlier program wedged mid-accept and did not
  reproduce after a fresh load. The fine-grained marker at word [12] exists
  to bisect that cheaply if it recurs.
- **Always check the mailbox magic before trusting any other word** — AXISRAM
  retains the previous run's data across loads.

---

## MEASURED RESULT — physical STM32N6570-DK, 2026-08-09 — **SUPERSEDED**

> **Pertains to the PREVIOUS image** (charge hash `0x3A72DADABEF89140`,
> image CRC32 `0x550FD984`). Real, kept in full, and no longer a
> validation of what is in `mailbox_burn.bin`. The map fingerprint and
> both domain words are unchanged; the header hash and CRC are what
> moved. Append a fresh measured section below after re-running.

Run by the lead developer; the build agent staged the binary and did
not take the board.

```
0x34178000 : 315A4351 00000002 00000004 00000000
0x34178010 : 8A112B8F 605F8ABE 550FD984 0009C469
0x34178020 : 000DC7F8 000853CB 0009F19E 00000004
0x34178030 : 00000006 00000048 00000031
```

Every expected value matches:

| Field | Expected | Measured |
|---|---|---|
| magic `QCZ1` | `0x315A4351` | ✓ |
| status | 2 = all passed | ✓ |
| passed / failed | 4 / 0 | ✓ |
| map fingerprint | `0x605F8ABE8A112B8F` | ✓ |
| progress / fine marker | 4 / 6 | ✓ |
| damage-responsive cells | 72 | ✓ (`0x48`) |
| **DECLINE cells** | 49 | ✓ (`0x31`) |

Image CRC32 recomputed on silicon: `0x550FD984`. DWT cycles:
639,593 · 902,136 · 545,227 · 651,678 — ≈ 2.74 M total ≈ **43 ms @
64 MHz** including the fail-closed refusals.

**Triple-target bit-determinism closed**: fingerprint
`0x605f8abe8a112b8f` is identical on x86-64, QEMU mps3-an547 and
physical STM32N657.

The two domain words are the ones worth reading. 72 cells change their
pull rate in response to *accumulated* damage — that is the harness's
whole thesis, confirmed on the deployed artifact rather than in the
solver. And 49 cells encode `DECLINE` (0xFF): states the solver could
not certify, where the executor stops instead of serving an arbitrary
action. That defect was caught by the QEMU rung, not by hosted tests.

Note for anyone reading a raw dump: words beyond [14] in this mailbox
are **stale data from a previous harness's run** — AXISRAM persists
across loads. Always check the magic first.


---

## RE-VERIFIED ON SILICON AFTER REMEDIATION — 2026-08-09

The gate-evaluability audit produced code changes to this harness, so
the image above was superseded and the board was re-run. Measured:

| Field | Expected | Measured |
|---|---|---|
| magic `QCZ1` | — | ✓ |
| status | 2 = all passed | ✓ |
| passed / failed | 4 / 0 | ✓ |
| table fingerprint | `0x605F8ABE8A112B8F` | ✓ |
| image CRC32 | `0xD983D89B` | ✓ |

fingerprint unchanged; params hash and CRC changed (codec coverage).

**Why the CRC is the word that matters on this re-run.** Five of the six
remediated harnesses changed only their header hash, not their solved
map — so the table fingerprint is identical before and after, and a
board still holding the *old* image would report a correct fingerprint
and look like a pass. The image CRC32 is the field that distinguishes
them, and it was checked on every one.
