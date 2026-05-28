#define _GNU_SOURCE
#include <sched.h>
#include <stdio.h>
#include <unistd.h>
int main(int argc, char **argv) {
  if (unshare(0x00000100) < 0) { perror("unshare"); return 1; }
  if (argc < 2) return 0;
  execvp(argv[1], argv + 1);
  perror("exec"); return 1;
}
