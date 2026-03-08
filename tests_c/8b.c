#include <stdlib.h>
#include <string.h>

void process(int* data) {
    // Use (void*)0 and sizeof expr (no parens)
    int* p = (void*)0;
    size_t s = sizeof *p; // Equivalent to sizeof(int)
    
    if (p == 0) { // 0 is also null in pointer context, though technically integer literal
        return;
    }
}

int main() {
    process((void*)0);
    return 0;
}
