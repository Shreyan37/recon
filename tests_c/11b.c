/*
 * buf.c  (NEW style)
 *
 * Semantically identical to buf_old.c; every difference is covered by a
 * Basic-level normalizer rule and should vanish under --semantic-diff=basic.
 *
 * Covered rules (same list as buf_old.c, opposite spellings):
 *   [basic]  (void*)0      ↔  NULL
 *   [basic]  fprintf(stdout,…) ↔  printf(…)
 *   [basic]  sizeof(*ptr)  ↔  sizeof(Type)
 *   [basic]  assert(cond)  — shared
 *   [basic]  x             ↔  !!x              (double negation eliminated)
 *   [basic]  a != b        ↔  !(a == b)        (positive comparison)
 *   [basic]  a >= b        ↔  !(a < b)         (positive comparison)
 *   [basic]  !done && !err ↔  !(done || err)   (De Morgan Form 2)
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
 * New: (void*)0 + sizeof(*b) + fprintf(stdout,…)
 */
void buf_init(Buffer *b) {
    assert(b != (void*)0);                         /* (void*)0  ↔ NULL */
    memset(b, 0, sizeof(*b));                      /* sizeof(*b) ↔ sizeof(Buffer) */
    b->active = 0;
    fprintf(stdout, "buf_init: capacity=%d\n", BUF_CAP); /* fprintf ↔ printf */
}

/* ── validate ──────────────────────────────────────────────────────────────
 * New: (void*)0 check, no double negation, positive inequality, positive ge
 */
int buf_validate(const Buffer *b, int max_len) {
    if (b == (void*)0)      return 0; /* (void*)0 ↔ NULL */
    if (b->active)          return 0; /* x ↔ !!x */
    if (b->len != 0)        return 0; /* a!=b  ↔  !(a==b) */
    if (max_len >= BUF_CAP) return 0; /* a>=b  ↔  !(a<b) */
    return 1;
}

/* ── process ───────────────────────────────────────────────────────────────
 * New: De Morgan — conjunction of negations !done && !err
 */
void buf_process(Buffer *b, int err) {
    assert(b != (void*)0);                         /* (void*)0 ↔ NULL */
    if (!b->done && !err) {                        /* De Morgan Form 2 */
        fprintf(stdout, "Processing %d bytes\n", b->len); /* fprintf ↔ printf */
        b->done = 1;
    }
}

/* ── reset ─────────────────────────────────────────────────────────────────
 * New: NULL check kept (both sides use NULL here — no diff expected),
 *      sizeof(Buffer) instead of sizeof(*b), (void*)0 assignment
 */
void buf_reset(Buffer *b, void **token) {
    if (b == NULL) return;                         /* NULL (same both sides) */
    memset(b, 0, sizeof(Buffer));                  /* sizeof(Buffer) ↔ sizeof(*b) */
    if (token != NULL) *token = (void*)0;          /* (void*)0 ↔ NULL */
    fprintf(stdout, "buf_reset: done\n");          /* fprintf ↔ printf */
}

/* ── report ────────────────────────────────────────────────────────────────
 * New: fprintf for logging
 */
void buf_report(const Buffer *b) {
    assert(b != (void*)0);
    fprintf(stdout, "len=%d active=%d done=%d\n",
            b->len, b->active, b->done);           /* fprintf ↔ printf */
}/*
 * buf.c  (NEW style)
 *
 * Semantically identical to buf_old.c; every difference is covered by a
 * Basic-level normalizer rule and should vanish under --semantic-diff=basic.
 *
 * Covered rules (same list as buf_old.c, opposite spellings):
 *   [basic]  (void*)0      ↔  NULL
 *   [basic]  fprintf(stdout,…) ↔  printf(…)
 *   [basic]  sizeof(*ptr)  ↔  sizeof(Type)
 *   [basic]  assert(cond)  — shared
 *   [basic]  x             ↔  !!x              (double negation eliminated)
 *   [basic]  a != b        ↔  !(a == b)        (positive comparison)
 *   [basic]  a >= b        ↔  !(a < b)         (positive comparison)
 *   [basic]  !done && !err ↔  !(done || err)   (De Morgan Form 2)
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
 * New: (void*)0 + sizeof(*b) + fprintf(stdout,…)
 */
void buf_init(Buffer *b) {
    assert(b != (void*)0);                         /* (void*)0  ↔ NULL */
    memset(b, 0, sizeof(*b));                      /* sizeof(*b) ↔ sizeof(Buffer) */
    b->active = 0;
    fprintf(stdout, "buf_init: capacity=%d\n", BUF_CAP); /* fprintf ↔ printf */
}

/* ── validate ──────────────────────────────────────────────────────────────
 * New: (void*)0 check, no double negation, positive inequality, positive ge
 */
int buf_validate(const Buffer *b, int max_len) {
    if (b == (void*)0)      return 0; /* (void*)0 ↔ NULL */
    if (b->active)          return 0; /* x ↔ !!x */
    if (b->len != 0)        return 0; /* a!=b  ↔  !(a==b) */
    if (max_len >= BUF_CAP) return 0; /* a>=b  ↔  !(a<b) */
    return 1;
}

/* ── process ───────────────────────────────────────────────────────────────
 * New: De Morgan — conjunction of negations !done && !err
 */
void buf_process(Buffer *b, int err) {
    assert(b != (void*)0);                         /* (void*)0 ↔ NULL */
    if (!b->done && !err) {                        /* De Morgan Form 2 */
        fprintf(stdout, "Processing %d bytes\n", b->len); /* fprintf ↔ printf */
        b->done = 1;
    }
}

/* ── reset ─────────────────────────────────────────────────────────────────
 * New: NULL check kept (both sides use NULL here — no diff expected),
 *      sizeof(Buffer) instead of sizeof(*b), (void*)0 assignment
 */
void buf_reset(Buffer *b, void **token) {
    if (b == NULL) return;                         /* NULL (same both sides) */
    memset(b, 0, sizeof(Buffer));                  /* sizeof(Buffer) ↔ sizeof(*b) */
    if (token != NULL) *token = (void*)0;          /* (void*)0 ↔ NULL */
    fprintf(stdout, "buf_reset: done\n");          /* fprintf ↔ printf */
}

/* ── report ────────────────────────────────────────────────────────────────
 * New: fprintf for logging
 */
void buf_report(const Buffer *b) {
    assert(b != (void*)0);
    fprintf(stdout, "len=%d active=%d done=%d\n",
            b->len, b->active, b->done);           /* fprintf ↔ printf */
}
