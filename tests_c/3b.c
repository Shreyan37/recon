#include <stdio.h>
#include <stddef.h>

int main() {
    int *ptr1 = ((void*)0);
    int *ptr2 = NULL;
    int *ptr3 = (void*)0;
    
    if (ptr1 == ((void*)0)) {
        printf("ptr1 is null\n");
    }
    
    if (ptr2 == NULL) {
        printf("ptr2 is null\n");
    }
    
    return 0;
}
