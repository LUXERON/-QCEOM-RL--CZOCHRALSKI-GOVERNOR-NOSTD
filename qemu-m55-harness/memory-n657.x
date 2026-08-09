/* RAM-only map for a debugger-loaded run on the physical STM32N657.
   Empirically (2026-08-08): bulk SWD downloads to the first AXISRAM
   megabyte (0x3400_0000) fail under CubeProgrammer, while the second
   megabyte accepts writes — so everything lives in 0x3410_0000+.
   Mailbox at 0x3417_8000 (top of the code region). */
MEMORY
{
  FLASH : ORIGIN = 0x34100000, LENGTH = 480K  /* code + vector table */
  RAM   : ORIGIN = 0x34180000, LENGTH = 512K  /* data + bss + stack */
}
