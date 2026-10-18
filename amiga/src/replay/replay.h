/*
 * ChipWave Lite - C interface to ptplayer (see replay.s)
 * Copyright (C) 2026 Del and the ChipWave contributors
 * GPL-3.0-or-later; see the header of replay.s. ptplayer is public domain.
 */
#ifndef CWL_REPLAY_H
#define CWL_REPLAY_H

#include <exec/types.h>

/* live player: CIA-B timed, audio channels allocated via audio.device */
LONG cwl_install(void);
void cwl_remove(void);
void cwl_init(APTR mod);
void cwl_end(void);

extern volatile UBYTE mt_Enable;     /* non-zero: play */
extern volatile UBYTE mt_E8Trigger;  /* last E8x value */
extern volatile UBYTE mt_SongEnd;    /* counts restarts of the song */

#ifndef CWL_NOLOG
/* tick-log player: same replay code, Paula/CIAs replaced by RAM shadows */
void cwl_log_init(APTR mod);
void cwl_log_end(void);
void cwl_log_music(void);
void cwl_log_dmaon(void);
void cwl_log_setrep(void);

extern UBYTE mtlog_custom[0x200];
extern UBYTE mtlog_ciaa[0x1000];
extern volatile UBYTE mtlog_E8Trigger;
extern volatile UBYTE mtlog_SongEnd;
extern volatile UBYTE mtlog_tempo;
#endif

#endif
