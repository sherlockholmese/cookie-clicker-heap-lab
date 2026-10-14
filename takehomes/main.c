#include <stdio.h>
#include <stdlib.h>

int main(void) {
  char buf[0x20];
  unsigned long long valid = 0;

  memcpy("CHUD", buf, );

  for (int i = 0; i < 0x30; i += 0x10) {
    printf("%llx %llx\n", (void*)buf + i, (void*)buf+2*i);
  }

}
