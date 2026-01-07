#include <stdint.h>
#include <stdio.h>

typedef struct Struct_A {
    uint32_t a;
    uint16_t b;
    uint32_t c;
    uint16_t d;
} Struct_A;

typedef struct Inner {
    uint32_t a;
    uint32_t b;
} Inner;

typedef struct Outer {
    Inner a;
    Inner b;
} Outer;

typedef struct Big_Outer {
    Outer a;
    uint8_t troll_a;
    uint8_t troll_b;
    Outer b;
} Big_Outer;

uint32_t fn_a(uint32_t a) { return a - 10; }

uint32_t fn_struct(Inner s) { return s.a + s.b; }

uint32_t fn_add(uint32_t a, uint32_t b) { return a + b; }

Struct_A fn_b(Struct_A a) {
    printf("Test Call: %d %d %d %d\n", a.a, a.b, a.c, a.d);
    Struct_A out = {0};
    out.a = a.c;
    out.b = a.d;

    out.c = a.a;
    out.d = a.b;
    return out;
}

uint32_t fn_c(Outer a) { return a.a.a + a.a.b + a.b.a + a.b.b; }

void fn_d(Big_Outer d) { printf("fn_d test call\n"); }