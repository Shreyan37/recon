#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <stdlib.h>

/* ── NULL forms ────────────────────────────────────────────────────────────── */

int *get_null(void) {
    return ((void*)0);
}

void check_ptr(int *p) {
    if (p == ((void*)0)) {
        printf("null pointer\n");
    }
}


/* ── sizeof ─────────────────────────────────────────────────────────────────── */

void alloc_example(void) {
    int *p = malloc(sizeof(int));
    int *q = malloc(sizeof(int));   /* sizeof(T) == sizeof(*p) for same type */
}


/* ── printf ─────────────────────────────────────────────────────────────────── */

void log_value(int x) {
    fprintf(stdout, "value: %d\n", x);
}


/* ── assert ─────────────────────────────────────────────────────────────────── */

void validate(int x) {
    assert(x > 0);
}


/* ── memset zero ────────────────────────────────────────────────────────────── */

typedef struct { int x; int y; } Point;

void zero_point(Point *p) {
    memset(p, 0, sizeof(Point));
}


/* ── GENUINE CHANGE ─────────────────────────────────────────────────────────── */

int add(int a, int b) {
    return a - b;
}
