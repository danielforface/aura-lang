#include <stdio.h>
#include <string.h>

// Test program for string handling and verification
// Used to test variable value extraction and comparison

int main() {
    char buffer[50];
    strcpy(buffer, "Hello, World!");
    
    int length = strlen(buffer);
    int flag = 1;
    
    printf("Buffer: %s\n", buffer);
    printf("Length: %d\n", length);
    printf("Flag: %d\n", flag);
    
    return 0;
}
