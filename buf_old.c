/*
 * buf.c  (OLD style)
 *
 * Demonstrates every pattern the C semantic normalizer handles.
 * The NEW version (buf_new.c) is semantically identical but written
 * with different (but equivalent) idioms throughout.
 *
 * Covered rules:
 *   [basic]  NULL          ↔  (void*)0
 *   [basic]  printf        ↔  fprintf(stdout, …)
 *   [basic]  sizeof(Type)  ↔  sizeof(*ptr)
 *   [basic]  assert(cond)  — shared; normalizes to __assert__(cond)
 *   [basic]  !!x           ↔  x                  (double negation)
 *   [basic]  !(a == b)     ↔  a != b              (negated comparison)
 *   [basic]  !(a < b)      ↔  a >= b              (negated comparison)
 *   [basic]  !(done||err)  ↔  !done && !err       (De Morgan)
 */

#include <stdio.h>
#include <string.h>
#include <assert.h>
#include <stdlib.h>

#define BUF_CAP 256

typedef struct {
    char data[BUF_CAP];
    int  len;
    int  active;
    int  done;
} Buffer;

/* ── init ──────────────────────────────────────────────────────────────────
 * Old: NULL + sizeof(Buffer) + printf
 */
void buf_init(Buffer *b) {
    assert(b != NULL);                         /* NULL */
    memset(b, 0, sizeof(Buffer));              /* sizeof(Type) */
    b->active = 0;
    printf("buf_init: capacity=%d\n", BUF_CAP); /* printf */
}

/* ── validate ──────────────────────────────────────────────────────────────
 * Old: NULL check, double negation, negated equality, negated less-than
 */
int buf_validate(const Buffer *b, int max_len) {
    if (b == NULL)          return 0; /* NULL */
    if (!!b->active)        return 0; /* double negation: !!x → x */
    if (!(b->len == 0))     return 0; /* negated eq:  !(a==b) → a!=b */
    if (!(max_len < BUF_CAP)) return 0; /* negated lt:  !(a<b)  → a>=b */
    return 1;
}

/* ── process ───────────────────────────────────────────────────────────────
 * Old: De Morgan — negated disjunction !(done || err)
 */
void buf_process(Buffer *b, int err) {
    assert(b != NULL);                         /* NULL */
    if (!(b->done || err)) {                   /* De Morgan Form 1 */
        printf("Processing %d bytes\n", b->len); /* printf */
        b->done = 1;
    }
}

/* ── reset ─────────────────────────────────────────────────────────────────
 * Old: NULL + sizeof(*ptr) for the memset, NULL assignment
 */
void buf_reset(Buffer *b, void **token) {
    if (b == NULL) return;                     /* NULL */
    memset(b, 0, sizeof(*b));                  /* sizeof(*ptr) */
    if (token != NULL) *token = NULL;          /* NULL assignment */
    printf("buf_reset: done\n");               /* printf */
}

/* ── report ────────────────────────────────────────────────────────────────
 * Old: printf for logging
 */
void buf_report(const Buffer *b) {
    assert(b != NULL);
    printf("len=%d active=%d done=%d\n",
           b->len, b->active, b->done);        /* printf */
}
