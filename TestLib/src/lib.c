#include <stdint.h>
#include <stdio.h>

typedef struct Struct_A {
    uint32_t a;
    uint16_t b;
    uint32_t c;
    uint16_t d;
} Struct_A;

Struct_A fn_b(Struct_A a) {
    printf("Test Call: %d %d %d %d\n", a.a, a.b, a.c, a.d);
    Struct_A out = {0};
    out.a = a.c;
    out.b = a.d;

    out.c = a.a;
    out.d = a.b;
    return out;
}