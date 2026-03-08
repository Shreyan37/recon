#include <stdio.h>
void greet(const char *name) {
    fprintf(stdout, "Hello, %s!\n", name);
    fprintf(stdout, "Count: %d\n", 42);
}
