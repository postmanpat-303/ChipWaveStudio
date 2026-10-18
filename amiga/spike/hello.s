; ChipWave Lite spike: OS-friendly hello (runs under vamos and on a real/emulated Amiga)
_LVOOpenLibrary  = -552
_LVOCloseLibrary = -414
_LVOOutput       = -60
_LVOWrite        = -48
        section code,code
start:  move.l  4.w,a6
        lea     dosname(pc),a1
        moveq   #36,d0                ; dos.library v36+ (OS 2.0+)
        jsr     _LVOOpenLibrary(a6)
        tst.l   d0
        beq.s   .fail
        move.l  d0,a6
        jsr     _LVOOutput(a6)
        move.l  d0,d1
        lea     msg(pc),a0
        move.l  a0,d2
        moveq   #msgend-msg,d3
        jsr     _LVOWrite(a6)
        move.l  a6,a1
        move.l  4.w,a6
        jsr     _LVOCloseLibrary(a6)
        moveq   #0,d0
        rts
.fail:  moveq   #20,d0
        rts
dosname: dc.b "dos.library",0
msg:    dc.b "ChipWave Lite spike: hello from 68020",10
msgend:
        even
