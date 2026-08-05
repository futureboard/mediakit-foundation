#include "cpu_features.h"

#if defined(__x86_64__) || defined(__i386__)

uint32_t mkff_cpu_detect_features(void) {
    __builtin_cpu_init();
    uint32_t features = 0;
    if (__builtin_cpu_supports("avx2")) {
        features |= MKFF_CPU_FEATURE_AVX2;
    }
    return features;
}

#else

uint32_t mkff_cpu_detect_features(void) {
    return 0;
}

#endif
