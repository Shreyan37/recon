#include <stdlib.h>
#include <string.h>

void process(int* data) {
    // Use NULL and sizeof(type)
    int* p = NULL;
    size_t s = sizeof(int);
    
    if (p == NULL) {
        return;
    }
}

int main() {
    process(NULL);
    return 0;
}
