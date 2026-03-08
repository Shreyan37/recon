#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void logic(void* ptr) {
    int* p = (void*)0;
    

    printf("Pointer is %p\n", (void*)p);

    int arr[10];
    size_t s = sizeof(arr);
}

int main() {
    logic((void*)0);
    return 0;
}
