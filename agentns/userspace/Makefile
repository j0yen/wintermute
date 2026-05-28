CC      := gcc
CFLAGS  := -O2 -Wall -Wextra -std=gnu11 -I..
BINS    := agent-wrap

all: $(BINS)

agent-wrap: agent-wrap.c ../include/uapi/linux/agent_namespaces.h
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f $(BINS)

.PHONY: all clean
