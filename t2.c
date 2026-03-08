#include <stdio.h>
#include <assert.h>
#include <string.h>
#include <stdlib.h>

void example(int *p, int x, int y) {
    // Null pointer – different form
    int *ptr = (void*)0;
    if (NULL == ptr) {   // reversed order
        printf("ptr is null\n");
    }

    // sizeof – different syntax
    size_t s = sizeof int;
    printf("sizeof int = %zu\n", s);

    // fprintf instead of printf
    fprintf(stdout, "x = %d, y = %d\n", x, y);

    // assert (unchanged)

    // memset zero (unchanged)

    // double negation – changed (behavioral)
    int flag = x;

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

    // De Morgan – alternate form of NAND
    if (!x || !y) {
        printf("nand\n");
    }
    if (!(x || y)) {
        printf("nor\n");
    }
}
