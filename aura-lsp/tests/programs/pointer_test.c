#include <stdio.h>
#include <stdlib.h>

// Test program with pointers for complex variable testing

int main() {
    int value = 42;
    int *ptr = &value;
    int **ptr_ptr = &ptr;
    
    printf("Value: %d\n", value);
    printf("Pointer value: %d\n", *ptr);
    printf("Double pointer value: %d\n", **ptr_ptr);
    
    return 0;
}
