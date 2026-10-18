# ChipWave Lite spike (2026-10-18)

These files are the feasibility spike behind [docs/AMIGA_LITE_PLAN.md](../../docs/AMIGA_LITE_PLAN.md). They are not part of the PC app build.

| File | What it does |
|---|---|
| `hello.s` | Writes a line through dos.library (vasm, hunk exe). Runs under vamos on Linux. |
| `beep.s` | Plays a square wave on Paula channel 0 by writing the hardware registers, then writes `DH0:beep-result.txt` (`BEEPOK` if audio DMA came on). |
| `cwlite_spike.c` | Opens a Workbench window, beeps through audio.device, then writes `DH0:c-spike-result.txt`. Built with m68k-amigaos-gcc and the NDK. |
| `run_spike.sh` | Builds everything, runs `hello` under vamos, makes a bootable ADF and boots it in FS-UAE with the built-in AROS ROM, then prints the results. |

Result on the box: `BEEPOK`, and `window=ok audio=ok`. GPL-3.0-or-later.
