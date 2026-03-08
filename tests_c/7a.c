// NULL vs (void*)0 in assignments and conditions
#include <stddef.h>
int main() {
    int *p = NULL;
    int *q = NULL;
    if (p == NULL) return 1;
    if (q == NULL) return 2;
    return 0;
}
