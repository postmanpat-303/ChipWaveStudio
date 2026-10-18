/*
 * ChipWave Lite - per-tick reference dump from libxmp (host tool, Linux)
 * Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
 * Uses libxmp (Claudio Matsuoka, Hipolito Carraro Jr and contributors, MIT licence).
 *
 *   cc -O2 xmpref.c -lxmp -o xmpref && ./xmpref song.mod > ref.txt
 *
 * One line per tick (frame): tick pos row frame bpm speed, then per channel
 * period (Amiga units) and volume (0..64) as libxmp's ProTracker mode plays it.
 */
#include <stdio.h>
#include <xmp.h>

int main(int argc, char **argv)
{
	xmp_context x;
	struct xmp_frame_info fi;
	int tick = 0, c;

	if (argc < 2) {
		fprintf(stderr, "usage: xmpref song.mod\n");
		return 2;
	}
	x = xmp_create_context();
	if (xmp_load_module(x, argv[1]) != 0) {
		fprintf(stderr, "libxmp can't load %s\n", argv[1]);
		return 1;
	}
	if (xmp_start_player(x, 44100, 0) != 0)
		return 1;
	xmp_set_player(x, XMP_PLAYER_MODE, XMP_MODE_PROTRACKER);
	for (;;) {
		if (xmp_play_frame(x) != 0)
			break;
		xmp_get_frame_info(x, &fi);
		if (fi.loop_count > 0)
			break;
		printf("%05d %02d %02d %02d %3d %2d", tick, fi.pos, fi.row, fi.frame, fi.bpm, fi.speed);
		for (c = 0; c < 4; c++)
			printf(" | p%03u v%02u", fi.channel_info[c].period >> 12,
				fi.channel_info[c].volume);
		printf("\n");
		tick++;
	}
	xmp_end_player(x);
	xmp_release_module(x);
	xmp_free_context(x);
	return 0;
}
