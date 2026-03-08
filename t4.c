#include <stdio.h>
#include <assert.h>
#include <string.h>

void example(int *p, int x, int y) {
    // NULL – different form
    int *ptr = ((void*)0);
    if (NULL == ptr) {
        fprintf(stdout, "ptr is null\n");
    }

    // sizeof – different syntax
    size_t s = sizeof int;
    fprintf(stdout, "sizeof int = %zu\n", s);

    // fprintf instead of printf
    fprintf(stdout, "x = %d, y = %d\n", x, y);

    // assert (unchanged)
    assert(x > 0);

    // memset zero (unchanged)
    memset(p, 0, sizeof(*p));

    // no double negation – same as file1
    int flag = x;

    // comparisons (unchanged)
    if (x == y) {
        printf("equal\n");
    }
    if (x != y) {
        printf("not equal\n");
    }
    if (x < y) {
        printf("less\n");
    }

    // De Morgan – alternate forms
    if (!x || !y) {
        printf("nand\n");
    }
    if (!x && !y) {
        printf("nor\n");
    }
}
