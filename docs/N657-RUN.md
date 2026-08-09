# STM32N657 Physical Run — Czochralski Pull-Map Executor

**Status: BUILT AND STAGED, NOT YET EXECUTED ON SILICON.** The board is a
shared serialized resource under lead-dev control, so this harness stops at
the built artifact. Everything below is ready to run verbatim; nothing in
this file is a claim about measured silicon.

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
| [6] | `+0x18` | image CRC32 recomputed on silicon | — |
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
