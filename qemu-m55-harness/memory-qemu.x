/* Memory map for QEMU's mps3-an547 (Arm SSE-300, Cortex-M55) — the
   STM32N6-class part. Same map as LUXERON/NO_STD-QEMU-TEST-HARNESS:
   vector table + code in ITCM at 0x0000_0000 (loadable via -kernel),
   data + stack in DTCM at 0x2000_0000. */
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 512K  /* ITCM — code + vector table */
  RAM   : ORIGIN = 0x20000000, LENGTH = 512K  /* DTCM — data + stack */
}
