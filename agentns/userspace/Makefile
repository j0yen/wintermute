CC      := gcc
CFLAGS  := -O2 -Wall -Wextra -std=gnu11 -I..
BINS    := agentns-unshare agent-wrap

all: $(BINS)

# Preferred creation path: prctl(PR_SET_AGENT_NS). Reachable from plain
# userspace; not bound to an (exhausted) clone-flag bit.
agentns-unshare: agentns-unshare.c ../include/uapi/linux/agent_namespaces.h
	$(CC) $(CFLAGS) -o $@ $<

# Legacy clone-flag wrapper. Kept for reference; the unshare(CLONE_NEWAGENT)
# path it uses cannot create a namespace (clone-flag space is exhausted).
# Use agentns-unshare instead.
agent-wrap: agent-wrap.c ../include/uapi/linux/agent_namespaces.h
	$(CC) $(CFLAGS) -o $@ $<

clean:
	rm -f $(BINS)

.PHONY: all clean
