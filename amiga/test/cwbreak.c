/*
 * ChipWave Lite - test helper: send Ctrl-C to a CLI command after a delay
 * Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
 *
 *   CWBreak SECONDS/N/A,COMMAND/A,RESULT/K
 *
 * Waits SECONDS, finds the CLI process running COMMAND (e.g. ChipWaveLite)
 * and signals SIGBREAKF_CTRL_C to it, like pressing Ctrl-C in its window.
 * Used by the headless tests (the AROS ROM has no Break command).
 */
#include <exec/types.h>
#include <dos/dos.h>
#include <dos/dosextens.h>
#include <proto/exec.h>
#include <proto/dos.h>

static int same(const UBYTE *bstr, const char *name)
{
	int n = bstr[0], i;
	const UBYTE *p = bstr + 1;
	/* compare the file part of the BCPL command name */
	for (i = n; i > 0; i--)
		if (p[i - 1] == ':' || p[i - 1] == '/')
			break;
	p += i;
	n -= i;
	for (i = 0; i < n; i++) {
		char a = p[i], b = name[i];
		if (!b) return 0;
		if (a >= 'a' && a <= 'z') a -= 32;
		if (b >= 'a' && b <= 'z') b -= 32;
		if (a != b) return 0;
	}
	return name[n] == 0;
}

int main(void)
{
	LONG args[3] = { 0 };
	struct RDArgs *rda = ReadArgs((CONST_STRPTR)"SECONDS/N/A,COMMAND/A,RESULT/K", args, NULL);
	LONG i, max, found = 0;
	if (!rda)
		return RETURN_FAIL;
	Delay(*(LONG *)args[0] * 50);
	Forbid();
	max = MaxCli();
	for (i = 1; i <= max; i++) {
		struct Process *p = FindCliProc(i);
		struct CommandLineInterface *cli;
		if (!p || (struct Task *)p == FindTask(NULL))
			continue;
		cli = BADDR(p->pr_CLI);
		if (cli && cli->cli_Module && cli->cli_CommandName &&
		    same(BADDR(cli->cli_CommandName), (const char *)args[1])) {
			Signal((struct Task *)p, SIGBREAKF_CTRL_C);
			found = i;
			break;
		}
	}
	Permit();
	if (args[2]) {
		BPTR fh = Open((CONST_STRPTR)args[2], MODE_NEWFILE);
		if (fh) {
			FPrintf(fh, (CONST_STRPTR)"ctrl_c_sent_to_cli=%ld\n", found);
			Close(fh);
		}
	}
	FreeArgs(rda);
	return found ? RETURN_OK : RETURN_WARN;
}
