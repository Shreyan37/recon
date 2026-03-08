#include <stdio.h>
#include <assert.h>
#include <string.h>
#include <stdlib.h>

void example(int *p, int x, int y) {
    int *ptr = NULL;
    if (ptr == NULL) {
        printf("ptr is null\n");
    }
    size_t s = sizeof(int);
    printf("sizeof int = %zu\n", s);
    printf("x = %d, y = %d\n", x, y);
    assert(x > 0);
    memset(p, 0, sizeof(*p));
    int flag = !!x;
    int z = 1*2*3*4;
    if (x == y) {
        printf("equal\n");
    }
    if (x != y) {
        printf("not equal\n");
    }
    if (x < y) {
        printf("less\n");
    }
    if (!(x && y)) {
        printf("nand\n");
    }
    if (!(x || y)) {
        printf("nor\n");
    }
}
