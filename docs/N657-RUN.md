# STM32N657 Physical Run — Czochralski Pull-Map Executor

**Status: REBUILT AND STAGED (third image), NOT YET EXECUTED ON
SILICON.** The board is a shared serialized resource under lead-dev
control, so this harness stops at the built artifact. Everything below is
ready to run verbatim.

> **BOTH measured-result sections at the bottom of this file are now
> SUPERSEDED.** They are kept in full — they were real runs — but neither
> validates what is currently in `mailbox_burn.bin`.
>
> **Third image (current), 2026-08-09 — the heat-transport hash defect.**
> A follow-up audit found `charge_hash` mixed the **stress** half of the
> physics and omitted the **heat-transport** half outright. `RHO_S`,
> `L_FUS`, `K_SOL`, `K_LIQ` and `T_MELT` were not hashed, even though
> `crystal::g_required` solves the Stefan balance
> `G_s = (ρ_s·L_f·v + k_l·G_l)/k_s` and hands that gradient straight to the
> stress model and therefore to the slip gate.
>
> Measured on the shipped gating charge: revising `K_SOL` from 22 to
> 25 W/(m·K) — a plausible literature revision — re-solves **61.8% of the
> 1296 map cells** and cuts **23.3% off the pull time** (21.92 h → 16.82 h),
> and across the falsifier's 12-charge corpus it moves **75.5% of 15,552
> cells** and flips one charge from DECLINED to a 34.92 h plan. Every one
> of those maps carried a **byte-identical `charge_hash`**, so every fielded
> puller would have `accept()`ed them. This was a silent **mis-serve**, not
> a false refusal, and it is the one outright hash defect found in the
> estate.
>
> Fixing it also required an omission guard, since a pinned hash literal is
> structurally blind to a constant that was never hashed. That guard
> (`every_declared_model_constant_is_hashed`, which scans `crystal.rs` for
> `pub const` and checks each is mixed in) immediately found two more:
> `SLIP_LIMIT` — revising it leaves the map identical while putting the
> declared 0.90 guard *above* the physical slip limit it protects — and
> `DT_H`, the characterization step, where 0.02 → 0.05 moves the governed
> pull's final damage 0.747 → 0.882, 18% of the whole budget.
>
> | Field | First image | Second image | **Current (third)** |
> |---|---|---|---|
> | Charge hash (header offset 16) | `0x3A72DADABEF89140` | `0xB493141295394B69` | **`0x46AD61386DE01E5D`** |
> | Stale-charge demo constant | `0xE260DB72161AB5BC` | `0xAD27333AF276C91E` | **`0x2B3F39053CBDC4E6`** |
> | Image CRC32 (mailbox word [6]) | `0x550FD984` | `0xD983D89B` | **`0x12F5CD74`** |
> | Map fingerprint (words [4,5]) | `0x605F8ABE8A112B8F` | unchanged | **unchanged** |
>
> The solved map has never changed across all three images — only its
> provenance binding, and therefore the header and the CRC. Host parity
> 4/4, NOSTD host 4/4 and QEMU mps3-an547 4/4 re-run on the new image.
> **The lead developer must re-flash `mailbox_burn.bin` and re-run.**
>
> ⚠️ **Read word [6], not words [4,5], to tell a fresh board from a stale
> one.** The map fingerprint is identical on all three images, so a board
> still holding either older image reports a *correct* fingerprint and
> looks like a pass. Only the CRC32 distinguishes them, and the expected
> value is now **`0x12F5CD74`**.

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
| [6] | `+0x18` | image CRC32 recomputed on silicon | **`0x12F5CD74`** ← the freshness word |
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

## RE-VERIFIED ON SILICON AFTER REMEDIATION — 2026-08-09 — **SUPERSEDED**

> **Pertains to the SECOND image** (charge hash `0xB493141295394B69`,
> image CRC32 `0xD983D89B`). Real, kept in full, and no longer a
> validation of what is in `mailbox_burn.bin` — the heat-transport hash
> defect described at the top of this file was found after this run and
> moved the header hash and CRC again. The map fingerprint and both
> domain words are unchanged. Append a fresh measured section below after
> re-running.

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

---

## PENDING RE-RUN — third image, 2026-08-09

Staged, not executed. The heat-transport hash defect (top of this file)
moved the header hash and CRC a third time. The lead developer owns the
board; this agent stopped at the built artifact.

| Field | Expected on the fresh image |
|---|---|
| magic `QCZ1` | `0x315A4351` |
| status | `2` = all passed |
| passed / failed | `4` / `0` |
| map fingerprint [4,5] | `0x605F8ABE8A112B8F` (**unchanged — cannot prove freshness**) |
| **image CRC32 [6]** | **`0x12F5CD74`** (**this is the freshness proof**) |
| progress [11] / marker [12] | `4` / `6` |
| damage-responsive cells [13] | `72` (`0x48`) |
| DECLINE cells [14] | `49` (`0x31`) |

If word [6] reads `0xD983D89B` or `0x550FD984`, the board is still
running an older image and the flash did not take — regardless of what
words [4,5] say.

Already re-verified off-board on the third image: hosted x86-64 18/18,
NOSTD host 4/4, QEMU mps3-an547 / Cortex-M55 **4/4** (72 responsive
cells, 49 declines — both unchanged, as expected for an unchanged map).
