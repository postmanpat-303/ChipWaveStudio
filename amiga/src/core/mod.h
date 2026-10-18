/*
 * ChipWave Lite - ProTracker .mod reader (portable C89, no OS calls)
 * Copyright (C) 2026 Del and the ChipWave contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
#ifndef CWL_MOD_H
#define CWL_MOD_H

/* Size of a 31-sample, 4-channel ProTracker module header. */
#define MOD_HEADER_SIZE 1084UL
#define MOD_PATTERN_SIZE 1024UL
#define MOD_NUM_SAMPLES 31

typedef struct ModSample {
	char name[23];          /* NUL terminated */
	unsigned long length;   /* bytes */
	unsigned char finetune; /* 0..15 */
	unsigned char volume;   /* 0..64 */
	unsigned long loop_start; /* bytes */
	unsigned long loop_len;   /* bytes; <= 2 means "no loop" */
	unsigned long offset;   /* byte offset of the sample data in the file */
} ModSample;

typedef struct ModInfo {
	char title[21];
	char tag[5];
	unsigned int song_length;   /* 1..128 positions */
	unsigned int restart;
	unsigned int num_patterns;  /* highest pattern in the order list + 1 */
	unsigned int num_samples_used;
	unsigned long sample_bytes; /* sum of all sample lengths */
	unsigned long needed_size;  /* header + patterns + samples */
	ModSample sample[MOD_NUM_SAMPLES];
} ModInfo;

enum {
	MOD_OK = 0,
	MOD_ERR_SHORT,      /* smaller than a header */
	MOD_ERR_TAG,        /* not M.K. / M!K! / FLT4 / 4CHN */
	MOD_ERR_LENGTH,     /* song length 0 or > 128 */
	MOD_ERR_PATTERN,    /* order list refers to pattern >= 100 */
	MOD_ERR_TRUNCATED   /* file is shorter than its patterns + samples */
};

/* Parse and validate a module in memory. Returns MOD_OK or an error code;
 * on MOD_ERR_TRUNCATED, info is filled in so the caller can report sizes. */
int mod_parse(const unsigned char *data, unsigned long size, ModInfo *info);

/* Do what ProTracker does after loading: clear the first word of every
 * sample, so the one-word "idle loop" of non-looped samples is silent.
 * Also clamps volume to 64 and loops that run past the sample end. */
void mod_prepare(unsigned char *data, ModInfo *info);

const char *mod_error_text(int err);

#endif
