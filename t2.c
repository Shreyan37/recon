#include <stdio.h>
#include <assert.h>
#include <string.h>
#include <stdlib.h>

void example(int *p, int x, int y) {
    int *ptr = (void*)0;
    if (NULL == ptr) {  
        printf("ptr is null\n");
    }

    size_t s = sizeof int;
    printf("sizeof int = %zu\n", s);

    fprintf(stdout, "x = %d, y = %d\n", x, y);

    int flag = x;
    int z = 4*2*3*1;
    if (x == y) {
        printf("equal\n");
    }
    if (x != y) {
        printf("not equal\n");
    }
    if (x < y) {
        printf("less\n");
    }
    if (!x || !y) {
        printf("nand\n");
    }
    if (!(x || y)) {
        printf("nor\n");
    }
}
