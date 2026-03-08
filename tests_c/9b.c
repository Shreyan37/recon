#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void logic(void* ptr) {
    int* p = NULL;
    fprintf(stdout, "Pointer is %p\n", (void*)p);

    int arr[10];
    size_t s = sizeof(arr);
}

int main() {
    logic(NULL);
    return 0;
}
