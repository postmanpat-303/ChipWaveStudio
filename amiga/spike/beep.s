; ChipWave Lite spike: Paula beep. Allocates the four audio channels through
; audio.device would be the clean way; this spike only grabs channel 0 briefly
; via the hardware registers, then writes a marker file so a headless run can
; check it got that far.
_LVOOpenLibrary  = -552
_LVOCloseLibrary = -414
_LVOAllocMem     = -198
_LVOFreeMem      = -210
_LVODelay        = -198
_LVOOpen         = -30
_LVOClose        = -36
_LVOWrite        = -48
_LVOOutput       = -60
MEMF_CHIP        = 2
CUSTOM           = $dff000
DMACON           = $096
DMACONR          = $002
AUD0LC           = $0a0
AUD0LEN          = $0a4
AUD0PER          = $0a6
AUD0VOL          = $0a8
        section code,code
start:  move.l  4.w,a6
        lea     dosname(pc),a1
        moveq   #36,d0
        jsr     _LVOOpenLibrary(a6)
        move.l  d0,dosbase
        beq     .fail
        ; 32-byte square wave in chip RAM
        moveq   #32,d0
        move.l  #MEMF_CHIP,d1
        jsr     _LVOAllocMem(a6)
        move.l  d0,wave
        beq     .fail
        move.l  d0,a0
        moveq   #15,d1
.hi:    move.b  #100,(a0)+
        dbf     d1,.hi
        moveq   #15,d1
.lo:    move.b  #-100,(a0)+
        dbf     d1,.lo
        lea     CUSTOM,a5
        move.l  wave,AUD0LC(a5)
        move.w  #16,AUD0LEN(a5)       ; words
        move.w  #220,AUD0PER(a5)      ; 3546895/(220*32) ~ 504 Hz (PAL)
        move.w  #48,AUD0VOL(a5)
        move.w  #$8201,DMACON(a5)     ; SET | DMAEN | AUD0EN
        move.w  DMACONR(a5),d7        ; read back: bit 0 should be set
        move.l  dosbase,a6
        moveq   #50,d1                ; 1 s
        jsr     _LVODelay(a6)
        move.w  #$0001,DMACON(a5)     ; AUD0 off
        move.w  #0,AUD0VOL(a5)
        ; marker file
        lea     outname(pc),a0
        move.l  a0,d1
        move.l  #1006,d2              ; MODE_NEWFILE
        jsr     _LVOOpen(a6)
        move.l  d0,d4
        beq.s   .free
        move.l  d4,d1
        lea     okmsg(pc),a0
        btst    #0,d7
        bne.s   .w
        lea     badmsg(pc),a0
.w:     move.l  a0,d2
        moveq   #6,d3
        jsr     _LVOWrite(a6)
        move.l  d4,d1
        jsr     _LVOClose(a6)
.free:  move.l  4.w,a6
        move.l  wave,a1
        moveq   #32,d0
        jsr     _LVOFreeMem(a6)
        move.l  dosbase,a1
        jsr     _LVOCloseLibrary(a6)
        moveq   #0,d0
        rts
.fail:  moveq   #20,d0
        rts
dosname: dc.b "dos.library",0
outname: dc.b "DH0:beep-result.txt",0
okmsg:  dc.b "BEEPOK"
badmsg: dc.b "NODMA!"
        even
        section bss,bss
dosbase: ds.l 1
wave:    ds.l 1
