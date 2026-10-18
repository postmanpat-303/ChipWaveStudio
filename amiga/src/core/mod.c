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
#include "mod.h"

static unsigned int be16(const unsigned char *p)
{
	return ((unsigned int)p[0] << 8) | p[1];
}

static void put16(unsigned char *p, unsigned int v)
{
	p[0] = (unsigned char)(v >> 8);
	p[1] = (unsigned char)v;
}

static void copy_text(char *dst, const unsigned char *src, int n)
{
	int i;
	for (i = 0; i < n; i++) {
		unsigned char c = src[i];
		if (c == 0)
			break;
		dst[i] = (c < 32 || c > 126) ? '.' : (char)c;
	}
	/* trim trailing blanks */
	while (i > 0 && dst[i - 1] == ' ')
		i--;
	dst[i] = 0;
}

int mod_parse(const unsigned char *data, unsigned long size, ModInfo *info)
{
	const unsigned char *t;
	unsigned int i, maxpat = 0;
	unsigned long off;

	if (size < MOD_HEADER_SIZE)
		return MOD_ERR_SHORT;

	t = data + 1080;
	if (!((t[0] == 'M' && t[1] == '.' && t[2] == 'K' && t[3] == '.') ||
	      (t[0] == 'M' && t[1] == '!' && t[2] == 'K' && t[3] == '!') ||
	      (t[0] == 'F' && t[1] == 'L' && t[2] == 'T' && t[3] == '4') ||
	      (t[0] == '4' && t[1] == 'C' && t[2] == 'H' && t[3] == 'N')))
		return MOD_ERR_TAG;
	for (i = 0; i < 4; i++)
		info->tag[i] = (char)t[i];
	info->tag[4] = 0;

	copy_text(info->title, data, 20);
	info->song_length = data[950];
	info->restart = data[951];
	if (info->song_length == 0 || info->song_length > 128)
		return MOD_ERR_LENGTH;
	for (i = 0; i < 128; i++) {
		unsigned int p = data[952 + i];
		if (p >= 100)
			return MOD_ERR_PATTERN;
		if (p > maxpat)
			maxpat = p;
	}
	info->num_patterns = maxpat + 1;

	off = MOD_HEADER_SIZE + (unsigned long)info->num_patterns * MOD_PATTERN_SIZE;
	info->sample_bytes = 0;
	info->num_samples_used = 0;
	for (i = 0; i < MOD_NUM_SAMPLES; i++) {
		const unsigned char *h = data + 20 + i * 30;
		ModSample *s = &info->sample[i];
		copy_text(s->name, h, 22);
		s->length = (unsigned long)be16(h + 22) * 2;
		s->finetune = h[24] & 15;
		s->volume = h[25];
		s->loop_start = (unsigned long)be16(h + 26) * 2;
		s->loop_len = (unsigned long)be16(h + 28) * 2;
		s->offset = off;
		off += s->length;
		info->sample_bytes += s->length;
		if (s->length > 2)
			info->num_samples_used++;
	}
	info->needed_size = off;
	if (size < off)
		return MOD_ERR_TRUNCATED;
	return MOD_OK;
}

void mod_prepare(unsigned char *data, ModInfo *info)
{
	unsigned int i;
	for (i = 0; i < MOD_NUM_SAMPLES; i++) {
		unsigned char *h = data + 20 + i * 30;
		ModSample *s = &info->sample[i];
		if (s->volume > 64) {
			s->volume = 64;
			h[25] = 64;
		}
		if (s->loop_len > 2 && s->loop_start + s->loop_len > s->length) {
			/* loop runs past the end: clamp it, as ProTracker does */
			if (s->loop_start >= s->length) {
				s->loop_start = 0;
				s->loop_len = 2;
			} else {
				s->loop_len = s->length - s->loop_start;
			}
			put16(h + 26, (unsigned int)(s->loop_start / 2));
			put16(h + 28, (unsigned int)(s->loop_len / 2));
		}
		if (s->length >= 2) {
			data[s->offset] = 0;
			data[s->offset + 1] = 0;
		}
	}
}

const char *mod_error_text(int err)
{
	switch (err) {
	case MOD_OK: return "ok";
	case MOD_ERR_SHORT: return "file is too small to be a .mod";
	case MOD_ERR_TAG: return "not a 4-channel ProTracker module (no M.K./M!K!/FLT4/4CHN tag)";
	case MOD_ERR_LENGTH: return "song length is 0 or more than 128 positions";
	case MOD_ERR_PATTERN: return "order list refers to a pattern number of 100 or more";
	case MOD_ERR_TRUNCATED: return "file is shorter than its patterns and samples";
	}
	return "unknown error";
}
