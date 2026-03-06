#include <stddef.h>
int main() {
    int *p = NULL;
    if (p == NULL) return 1;
    void *q = (void*)0;
    return 0;
}
