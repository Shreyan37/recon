#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <stdlib.h>

/* ── NULL forms ────────────────────────────────────────────────────────────── */

int *get_null(void) {
    return NULL;
}

void check_ptr(int *p) {
    if (p == NULL) {
        printf("null pointer\n");
    }
}


/* ── sizeof ─────────────────────────────────────────────────────────────────── */

void alloc_example(void) {
    int *p = malloc(sizeof(int));
    int *q = malloc(sizeof(*p));
}


/* ── printf ─────────────────────────────────────────────────────────────────── */

void log_value(int x) {
    printf("value: %d\n", x);
}


/* ── assert ─────────────────────────────────────────────────────────────────── */

void validate(int x) {
    assert(x > 0);
}



typedef struct { int x; int y; } Point;

void zero_point(Point *p) {
    memset(p, 0, sizeof(*p));
}

int add(int a, int b) {
    return a + b;
}
