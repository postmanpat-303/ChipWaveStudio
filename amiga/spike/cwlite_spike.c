/* ChipWave Lite feasibility spike (C, amiga-gcc + NDK, AmigaOS 3.x / AROS).
 * - opens a small Workbench window titled "ChipWave Lite spike"
 * - plays a 1 s square-wave beep on Paula channel 0 through audio.device
 *   (the OS-friendly way: channel allocation, CMD_WRITE, chip RAM sample)
 * - writes DH0:c-spike-result.txt so a headless emulator run can be checked
 * GPL-3.0-or-later, part of ChipWave Studio. */
#include <exec/exec.h>
#include <devices/audio.h>
#include <intuition/intuition.h>
#include <dos/dos.h>
#include <proto/exec.h>
#include <proto/dos.h>
#include <proto/intuition.h>

struct IntuitionBase *IntuitionBase;

static UBYTE chans[] = {1, 2, 4, 8}; /* any one channel */

int main(void)
{
    BPTR out;
    struct Window *win = NULL;
    struct MsgPort *port;
    struct IOAudio *io;
    BYTE *wave;
    int i, audio_ok = 0, win_ok = 0;

    Printf("ChipWave Lite C spike (68020)\n");
    IntuitionBase = (struct IntuitionBase *)OpenLibrary("intuition.library", 36);
    if (IntuitionBase) {
        win = OpenWindowTags(NULL, WA_Title, (ULONG)"ChipWave Lite spike", WA_Left, 40, WA_Top, 30,
                             WA_Width, 320, WA_Height, 80, WA_DragBar, TRUE, WA_DepthGadget, TRUE,
                             WA_CloseGadget, TRUE, WA_Activate, TRUE, TAG_END);
        win_ok = win != NULL;
    }
    wave = AllocMem(32, MEMF_CHIP | MEMF_PUBLIC);
    port = CreateMsgPort();
    io = port ? (struct IOAudio *)CreateIORequest(port, sizeof(struct IOAudio)) : NULL;
    if (wave && io) {
        for (i = 0; i < 32; i++) wave[i] = i < 16 ? 100 : -100;
        io->ioa_Request.io_Message.mn_Node.ln_Pri = 127;
        io->ioa_Data = chans;
        io->ioa_Length = sizeof(chans);
        io->ioa_AllocKey = 0;
        if (OpenDevice((STRPTR)AUDIONAME, 0, (struct IORequest *)io, 0) == 0) {
            io->ioa_Request.io_Command = CMD_WRITE;
            io->ioa_Request.io_Flags = ADIOF_PERVOL;
            io->ioa_Data = (UBYTE *)wave;
            io->ioa_Length = 32;
            io->ioa_Period = 220;  /* ~504 Hz on PAL */
            io->ioa_Volume = 48;
            io->ioa_Cycles = 504;  /* ~1 s */
            audio_ok = DoIO((struct IORequest *)io) == 0;
            CloseDevice((struct IORequest *)io);
        }
    }
    Printf("window %s, audio %s\n", (ULONG)(win_ok ? "ok" : "FAIL"), (ULONG)(audio_ok ? "ok" : "FAIL"));
    if ((out = Open("DH0:c-spike-result.txt", MODE_NEWFILE))) {
        FPrintf(out, "window=%s audio=%s\n", (ULONG)(win_ok ? "ok" : "fail"), (ULONG)(audio_ok ? "ok" : "fail"));
        Close(out);
    }
    if (io) DeleteIORequest((struct IORequest *)io);
    if (port) DeleteMsgPort(port);
    if (wave) FreeMem(wave, 32);
    if (win) CloseWindow(win);
    if (IntuitionBase) CloseLibrary((struct Library *)IntuitionBase);
    return audio_ok && win_ok ? 0 : 5;
}
