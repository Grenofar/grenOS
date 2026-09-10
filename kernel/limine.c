#include "limine.h"

// Limine boot protocol requests
volatile struct limine_bootloader_info_request bootloader_info_request = {
    .id = {0xf68bf81b5b5b5b5b, 0x5b5b5b5bf68bf81b},
    .revision = 0,
    .response = NULL
};

volatile struct limine_memmap_request memmap_request = {
    .id = {0x67cf3d9d378a806f, 0xe304acdfc50c3c62},
    .revision = 0,
    .response = NULL
};

volatile struct limine_framebuffer_request framebuffer_request = {
    .id = {0x9d5827dcd881dd75, 0xa3148604f6fab11b},
    .revision = 0,
    .response = NULL
};

volatile struct limine_serial_request serial_request = {
    .id = {0x07e1f5b5b5b5b5b5, 0x5b5b5b5b07e1f5b5},
    .revision = 0,
    .response = NULL
};
