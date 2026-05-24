CC      := gcc
CFLAGS  := -O2 -Wall -Wextra -std=gnu11 -I..
BINS    := test_unshare unshare-helper

all: $(BINS)

test_unshare: test_unshare.c ../include/uapi/linux/agent_namespaces.h
	$(CC) $(CFLAGS) -o $@ $<

unshare-helper: unshare-helper.c
	$(CC) $(CFLAGS) -o $@ $<

unshare-helper.c:
	@printf '%s\n' \
		'#define _GNU_SOURCE' \
		'#include <sched.h>' \
		'#include <stdio.h>' \
		'#include <unistd.h>' \
		'int main(int argc, char **argv) {' \
		'  if (unshare(0x00000100) < 0) { perror("unshare"); return 1; }' \
		'  if (argc < 2) return 0;' \
		'  execvp(argv[1], argv + 1);' \
		'  perror("exec"); return 1;' \
		'}' > $@

check: all
	@./test_unshare && \
	 ./test_inheritance.sh && \
	 ./test_bpf_tracepoints.sh && \
	 echo "ALL TESTS PASS"

clean:
	rm -f $(BINS) unshare-helper.c
