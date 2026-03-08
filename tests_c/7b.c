// NULL vs (void*)0 in assignments and conditions
#include <stddef.h>
int main() {
    int *p = (void*)0;
    int *q = ((void*)0);
    if (p == (void*)0) return 1;
    if (q == ((void*)0)) return 2;
    return 0;
}
