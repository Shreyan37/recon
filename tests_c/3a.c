#include <stdio.h>
#include <stddef.h>

int main() {
    int *ptr1 = NULL;
    int *ptr2 = ((void*)0);
    int *ptr3 = (void*)0;
    
    if (ptr1 == NULL) {
        printf("ptr1 is null\n");
    }
    
    if (ptr2 == ((void*)0)) {
        printf("ptr2 is null\n");
    }
    
    return 0;
}
