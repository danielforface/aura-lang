#include "aura_rt.h"

// Stage 5.2 (prototype): runtime-provided C main.
// Later phases will initialize the async scheduler and capability handlers here.
int main(void) {
    return aura_entry();
}
