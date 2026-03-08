#include <stdio.h>
#include <assert.h>
#include <string.h>
#include <stdlib.h>

void example(int *p, int x, int y) {
    // Null pointer
    int *ptr = NULL;
    if (ptr == NULL) {
        printf("ptr is null\n");
    }

    // sizeof
    size_t s = sizeof(int);
    printf("sizeof int = %zu\n", s);

    // printf
    printf("x = %d, y = %d\n", x, y);

    // assert
    assert(x > 0);

    // memset zero
    memset(p, 0, sizeof(*p));

    // double negation
    int flag = !!x;

    // comparisons
    if (x == y) {
        printf("equal\n");
    }
    if (x != y) {
        printf("not equal\n");
    }
    if (x < y) {
        printf("less\n");
    }

    // De Morgan
    if (!(x && y)) {
        printf("nand\n");
    }
    if (!(x || y)) {
        printf("nor\n");
    }
}
